use std::collections::{HashMap, HashSet};

use dt_common::error::Error;
use dt_connector::meta_fetcher::{
    mysql::mysql_struct_check_fetcher::MysqlStructCheckFetcher,
    pg::pg_struct_check_fetcher::PgStructCheckFetcher,
};

use super::{base_test_runner::BaseTestRunner, rdb_test_runner::RdbTestRunner};

pub struct RdbStructTestRunner {
    pub base: RdbTestRunner,
}

impl RdbStructTestRunner {
    pub async fn new(relative_test_dir: &str) -> Result<Self, Error> {
        let base = RdbTestRunner::new(relative_test_dir).await.unwrap();
        Ok(Self { base })
    }

    pub async fn run_mysql_struct_test(&mut self) -> Result<(), Error> {
        self.base.execute_test_ddl_sqls().await?;
        self.base.base.start_task().await?;

        let expect_ddl_sqls = self.load_expect_ddl_sqls();
        let src_check_fetcher = MysqlStructCheckFetcher {
            conn_pool: self.base.src_conn_pool_mysql.as_mut().unwrap().clone(),
        };
        let dst_check_fetcher = MysqlStructCheckFetcher {
            conn_pool: self.base.dst_conn_pool_mysql.as_mut().unwrap().clone(),
        };

        let get_sql_lines = |sql: &str| -> HashSet<String> {
            let mut line_set = HashSet::new();
            let lines: Vec<&str> = sql.split("\n").collect();
            for line in lines {
                line_set.insert(line.trim_end_matches(",").to_owned());
            }
            line_set
        };

        let (src_db_tbs, dst_db_tbs) = self.base.get_compare_db_tbs().await.unwrap();
        for i in 0..src_db_tbs.len() {
            let src_ddl_sql = src_check_fetcher
                .fetch_table(&src_db_tbs[i].0, &src_db_tbs[i].1)
                .await;
            let dst_ddl_sql = dst_check_fetcher
                .fetch_table(&dst_db_tbs[i].0, &dst_db_tbs[i].1)
                .await;
            let key = format!("{}.{}", &dst_db_tbs[i].0, &dst_db_tbs[i].1);
            let expect_ddl_sql = expect_ddl_sqls.get(&key).unwrap().to_owned();

            println!("src_ddl_sql: {}", src_ddl_sql);
            println!("dst_ddl_sql: {}", dst_ddl_sql);
            println!("expect_ddl_sql: {}", expect_ddl_sql);
            // show create table may return sqls with indexes in different orders during tests,
            // so here we just compare all lines of the sqls.
            let dst_ddl_sql_lines = get_sql_lines(&dst_ddl_sql);
            let expect_ddl_sql_lines = get_sql_lines(&expect_ddl_sql);
            println!("dst_ddl_sql_lines: {:?}", dst_ddl_sql_lines);
            println!("expect_ddl_sql_lines: {:?}", expect_ddl_sql_lines);
            assert_eq!(dst_ddl_sql_lines, expect_ddl_sql_lines);
        }
        Ok(())
    }

    pub async fn run_pg_struct_test(&mut self) -> Result<(), Error> {
        self.base.execute_test_ddl_sqls().await?;
        self.base.base.start_task().await?;

        let src_check_fetcher = PgStructCheckFetcher {
            conn_pool: self.base.src_conn_pool_pg.as_mut().unwrap().clone(),
        };
        let dst_check_fetcher = PgStructCheckFetcher {
            conn_pool: self.base.dst_conn_pool_pg.as_mut().unwrap().clone(),
        };

        let (src_db_tbs, dst_db_tbs) = self.base.get_compare_db_tbs().await.unwrap();
        for i in 0..src_db_tbs.len() {
            let src_table = src_check_fetcher
                .fetch_table(&src_db_tbs[i].0, &src_db_tbs[i].1)
                .await;
            let dst_table = dst_check_fetcher
                .fetch_table(&dst_db_tbs[i].0, &dst_db_tbs[i].1)
                .await;
            println!("src_table: {:?}", src_table);
            println!("dst_table: {:?}", dst_table);
            assert_eq!(src_table, dst_table);
        }
        Ok(())
    }

    fn load_expect_ddl_sqls(&self) -> HashMap<String, String> {
        let mut ddl_sqls = HashMap::new();
        let ddl_file = format!("{}/expect_ddl.sql", self.base.base.test_dir);
        let lines = BaseTestRunner::load_file(&ddl_file);
        let mut lines = lines.iter().peekable();

        while let Some(line) = lines.next() {
            if line.trim().is_empty() {
                continue;
            }

            let table = line.trim().to_owned();
            let mut sql = String::new();
            while let Some(line) = lines.next() {
                if line.trim().is_empty() {
                    break;
                }
                sql.push_str(line);
                sql.push('\n');
            }
            ddl_sqls.insert(table, sql.trim().to_owned());
        }
        ddl_sqls
    }
}
