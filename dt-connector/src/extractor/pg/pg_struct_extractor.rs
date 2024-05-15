use async_trait::async_trait;
use dt_common::{log_info, rdb_filter::RdbFilter};

use dt_common::meta::{
    ddl_data::DdlData, ddl_type::DdlType, dt_data::DtData, position::Position,
    struct_meta::statement::struct_statement::StructStatement,
};

use sqlx::{Pool, Postgres};

use crate::close_conn_pool;
use crate::{
    extractor::base_extractor::BaseExtractor, meta_fetcher::pg::pg_struct_fetcher::PgStructFetcher,
    Extractor,
};

pub struct PgStructExtractor {
    pub base_extractor: BaseExtractor,
    pub conn_pool: Pool<Postgres>,
    pub schema: String,
    pub filter: RdbFilter,
}

#[async_trait]
impl Extractor for PgStructExtractor {
    async fn extract(&mut self) -> anyhow::Result<()> {
        log_info!("PgStructExtractor starts, schema: {}", self.schema);
        self.extract_internal().await?;
        self.base_extractor.wait_task_finish().await
    }

    async fn close(&mut self) -> anyhow::Result<()> {
        close_conn_pool!(self)
    }
}

impl PgStructExtractor {
    pub async fn extract_internal(&mut self) -> anyhow::Result<()> {
        let mut pg_fetcher = PgStructFetcher {
            conn_pool: self.conn_pool.to_owned(),
            schema: self.schema.clone(),
            filter: Some(self.filter.to_owned()),
        };

        // schema
        let schema_statement = pg_fetcher.get_create_schema_statement().await.unwrap();
        let statement = StructStatement::PgCreateSchema {
            statement: schema_statement,
        };
        self.push_dt_data(statement).await;

        // tables
        for statement in pg_fetcher.get_create_table_statements("").await.unwrap() {
            self.push_dt_data(StructStatement::PgCreateTable { statement })
                .await;
        }
        Ok(())
    }

    pub async fn push_dt_data(&mut self, statement: StructStatement) {
        let ddl_data = DdlData {
            schema: self.schema.clone(),
            tb: String::new(),
            query: String::new(),
            statement: Some(statement),
            ddl_type: DdlType::Unknown,
        };

        self.base_extractor
            .push_dt_data(DtData::Ddl { ddl_data }, Position::None)
            .await
            .unwrap()
    }
}
