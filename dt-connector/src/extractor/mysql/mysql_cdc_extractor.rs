use async_recursion::async_recursion;
use async_trait::async_trait;
use std::collections::HashMap;

use dt_meta::{
    adaptor::mysql_col_value_convertor::MysqlColValueConvertor, col_value::ColValue,
    ddl_data::DdlData, ddl_type::DdlType, dt_data::DtData,
    mysql::mysql_meta_manager::MysqlMetaManager, position::Position, row_data::RowData,
    row_type::RowType, sql_parser::ddl_parser::DdlParser,
};
use mysql_binlog_connector_rust::{
    binlog_client::BinlogClient,
    event::{
        event_data::EventData, event_header::EventHeader, query_event::QueryEvent,
        row_event::RowEvent, table_map_event::TableMapEvent,
    },
};

use dt_common::{error::Error, log_error, log_info, utils::rdb_filter::RdbFilter};

use crate::{
    datamarker::traits::DataMarkerFilter, extractor::base_extractor::BaseExtractor, Extractor,
};

pub struct MysqlCdcExtractor {
    pub base_extractor: BaseExtractor,
    pub meta_manager: MysqlMetaManager,
    pub filter: RdbFilter,
    pub url: String,
    pub binlog_filename: String,
    pub binlog_position: u32,
    pub server_id: u64,
    pub datamarker_filter: Option<Box<dyn DataMarkerFilter + Send>>,
}

const QUERY_BEGIN: &str = "BEGIN";

#[async_trait]
impl Extractor for MysqlCdcExtractor {
    async fn extract(&mut self) -> Result<(), Error> {
        log_info!(
            "MysqlCdcExtractor starts, binlog_filename: {}, binlog_position: {}",
            self.binlog_filename,
            self.binlog_position
        );
        self.extract_internal().await
    }
}

impl MysqlCdcExtractor {
    async fn extract_internal(&mut self) -> Result<(), Error> {
        let mut client = BinlogClient {
            url: self.url.clone(),
            binlog_filename: self.binlog_filename.clone(),
            binlog_position: self.binlog_position,
            server_id: self.server_id,
        };
        let mut stream = client.connect().await.unwrap();
        let mut table_map_event_map = HashMap::new();
        let mut binlog_filename = self.binlog_filename.clone();

        loop {
            let (header, data) = stream.read().await.unwrap();
            match data {
                EventData::Rotate(r) => {
                    binlog_filename = r.binlog_filename;
                }

                _ => {
                    self.parse_events(header, data, &binlog_filename, &mut table_map_event_map)
                        .await?
                }
            }
        }
    }

    #[async_recursion]
    async fn parse_events(
        &mut self,
        header: EventHeader,
        data: EventData,
        binlog_filename: &str,
        table_map_event_map: &mut HashMap<u64, TableMapEvent>,
    ) -> Result<(), Error> {
        let position = Position::MysqlCdc {
            // TODO, get server_id from source mysql
            server_id: String::new(),
            binlog_filename: binlog_filename.into(),
            next_event_position: header.next_event_position,
            timestamp: Position::format_timestamp_millis(header.timestamp as i64 * 1000),
        };

        match data {
            EventData::TableMap(d) => {
                table_map_event_map.insert(d.table_id, d);
            }

            EventData::TransactionPayload(event) => {
                for (mut inner_header, data) in event.uncompressed_events {
                    // headers of uncompressed events have no next_event_position,
                    // use header of TransactionPayload instead
                    inner_header.next_event_position = header.next_event_position;
                    self.parse_events(inner_header, data, binlog_filename, table_map_event_map)
                        .await?;
                }
            }

            EventData::WriteRows(mut w) => {
                for event in w.rows.iter_mut() {
                    let table_map_event = table_map_event_map.get(&w.table_id).unwrap();
                    if self.filter_event(table_map_event, RowType::Insert) {
                        continue;
                    }

                    let col_values = self
                        .parse_row_data(table_map_event, &w.included_columns, event)
                        .await?;
                    let row_data = RowData::new(
                        table_map_event.database_name.clone(),
                        table_map_event.table_name.clone(),
                        RowType::Insert,
                        None,
                        Some(col_values),
                    );
                    self.push_row_to_buf(row_data, position.clone()).await?;
                }
            }

            EventData::UpdateRows(mut u) => {
                for event in u.rows.iter_mut() {
                    let table_map_event = table_map_event_map.get(&u.table_id).unwrap();
                    if self.filter_event(table_map_event, RowType::Update) {
                        continue;
                    }

                    let col_values_before = self
                        .parse_row_data(table_map_event, &u.included_columns_before, &mut event.0)
                        .await?;
                    let col_values_after = self
                        .parse_row_data(table_map_event, &u.included_columns_after, &mut event.1)
                        .await?;
                    let row_data = RowData::new(
                        table_map_event.database_name.clone(),
                        table_map_event.table_name.clone(),
                        RowType::Update,
                        Some(col_values_before),
                        Some(col_values_after),
                    );
                    self.push_row_to_buf(row_data, position.clone()).await?;
                }
            }

            EventData::DeleteRows(mut d) => {
                for event in d.rows.iter_mut() {
                    let table_map_event = table_map_event_map.get(&d.table_id).unwrap();
                    if self.filter_event(table_map_event, RowType::Delete) {
                        continue;
                    }

                    let col_values = self
                        .parse_row_data(table_map_event, &d.included_columns, event)
                        .await?;
                    let row_data = RowData::new(
                        table_map_event.database_name.clone(),
                        table_map_event.table_name.clone(),
                        RowType::Delete,
                        Some(col_values),
                        None,
                    );
                    self.push_row_to_buf(row_data, position.clone()).await?;
                }
            }

            EventData::Query(query) => {
                self.handle_query_event(query, position.clone()).await?;
            }

            EventData::Xid(xid) => {
                let commit = DtData::Commit {
                    xid: xid.xid.to_string(),
                };

                if match &mut self.datamarker_filter {
                    Some(f) => f.filter_dtdata(&commit)?,
                    _ => false,
                } {
                    return Ok(());
                }

                self.base_extractor
                    .push_dt_data(commit, position.clone())
                    .await?;
            }

            _ => {}
        }

        Ok(())
    }

    async fn push_row_to_buf(
        &mut self,
        row_data: RowData,
        position: Position,
    ) -> Result<(), Error> {
        if match &mut self.datamarker_filter {
            // update the 'do_transaction_filter' flag in a transaction with DataMarkerFilter
            Some(f) => f.filter_rowdata(&row_data)?,
            None => false,
        } {
            return Ok(());
        }

        self.base_extractor.push_row(row_data, position).await
    }

    async fn parse_row_data(
        &mut self,
        table_map_event: &TableMapEvent,
        included_columns: &Vec<bool>,
        event: &mut RowEvent,
    ) -> Result<HashMap<String, ColValue>, Error> {
        let tb_meta = self
            .meta_manager
            .get_tb_meta(&table_map_event.database_name, &table_map_event.table_name)
            .await?;

        if included_columns.len() != event.column_values.len() {
            return Err(Error::ExtractorError(
                "included_columns not match column_values in binlog".into(),
            ));
        }

        let mut data = HashMap::new();
        for i in (0..tb_meta.basic.cols.len()).rev() {
            let key = tb_meta.basic.cols.get(i).unwrap();
            if let Some(false) = included_columns.get(i) {
                data.insert(key.clone(), ColValue::None);
                continue;
            }

            let col_type = tb_meta.col_type_map.get(key).unwrap();
            let raw_value = event.column_values.remove(i);
            let value = MysqlColValueConvertor::from_binlog(col_type, raw_value)?;
            data.insert(key.clone(), value);
        }
        Ok(data)
    }

    async fn handle_query_event(
        &mut self,
        query: QueryEvent,
        position: Position,
    ) -> Result<(), Error> {
        // TODO, currently we do not parse ddl if filtered,
        // but we should always try to parse ddl in the future
        if self.filter.filter_ddl() {
            return Ok(());
        }

        if query.query == QUERY_BEGIN {
            return Ok(());
        }

        log_info!("received ddl: {:?}", query);
        let mut ddl_data = DdlData {
            schema: query.schema,
            tb: String::new(),
            query: query.query.clone(),
            ddl_type: DdlType::Unknown,
            statement: None,
        };

        let parse_result = DdlParser::parse(&query.query);
        if let Err(error) = parse_result {
            // clear all metadata cache
            self.meta_manager.invalidate_cache("", "");
            log_error!(
                    "failed to parse ddl, will try ignore it, please execute the ddl manually in target, sql: {}, error: {}",
                    ddl_data.query,
                    error
                );
            return Ok(());
        }

        // case 1, execute: use db_1; create table tb_1(id int);
        // binlog query.schema == db_1, schema from DdlParser == None
        // case 2, execute: create table db_1.tb_1(id int);
        // binlog query.schema == empty, schema from DdlParser == db_1
        // case 3, execute: use db_1; create table db_2.tb_1(id int);
        // binlog query.schema == db_1, schema from DdlParser == db_2
        let (ddl_type, schema, tb) = parse_result.unwrap();
        ddl_data.ddl_type = ddl_type;
        if let Some(schema) = schema {
            ddl_data.schema = schema;
        }
        if let Some(tb) = tb {
            ddl_data.tb = tb;
        }

        // invalidate metadata cache
        self.meta_manager
            .invalidate_cache(&ddl_data.schema, &ddl_data.tb);

        let filter = if ddl_data.tb.is_empty() {
            self.filter.filter_db(&ddl_data.schema)
        } else {
            self.filter.filter_tb(&ddl_data.schema, &ddl_data.tb)
        };

        if !self.filter.filter_ddl() && !filter {
            self.base_extractor
                .push_dt_data(DtData::Ddl { ddl_data }, position)
                .await
                .unwrap();
        }
        Ok(())
    }

    fn filter_event(&mut self, table_map_event: &TableMapEvent, row_type: RowType) -> bool {
        self.filter.filter_event(
            &table_map_event.database_name,
            &table_map_event.table_name,
            &row_type.to_string(),
        )
    }
}
