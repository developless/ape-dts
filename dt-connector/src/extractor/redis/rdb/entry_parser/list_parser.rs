use anyhow::bail;
use dt_common::error::Error;
use dt_common::meta::redis::redis_object::{ListObject, RedisString};

use crate::extractor::redis::rdb::reader::rdb_reader::RdbReader;

const QUICKLIST_NODE_CONTAINER_PLAIN: u64 = 1;
const QUICKLIST_NODE_CONTAINER_PACKED: u64 = 2;

pub struct ListParser {}

impl ListParser {
    pub fn load_from_buffer(
        reader: &mut RdbReader,
        key: RedisString,
        type_byte: u8,
    ) -> anyhow::Result<ListObject> {
        let mut obj = ListObject::new();
        obj.key = key;

        match type_byte {
            super::RDB_TYPE_LIST => Self::read_list(&mut obj, reader)?,
            super::RDB_TYPE_LIST_ZIPLIST => obj.elements = reader.read_zip_list()?,
            super::RDB_TYPE_LIST_QUICKLIST => Self::read_quick_list(&mut obj, reader)?,
            super::RDB_TYPE_LIST_QUICKLIST_2 => Self::read_quick_list_2(&mut obj, reader)?,
            _ => {
                bail! {Error::RedisRdbError(format!(
                    "unknown list type {}",
                    type_byte
                ))}
            }
        }
        Ok(obj)
    }

    fn read_list(obj: &mut ListObject, reader: &mut RdbReader) -> anyhow::Result<()> {
        let size = reader.read_length()?;
        for _ in 0..size {
            let ele = reader.read_string()?;
            obj.elements.push(ele);
        }
        Ok(())
    }

    fn read_quick_list(obj: &mut ListObject, reader: &mut RdbReader) -> anyhow::Result<()> {
        let size = reader.read_length()?;
        for _ in 0..size {
            let zip_list_elements = reader.read_zip_list()?;
            obj.elements.extend(zip_list_elements);
        }
        Ok(())
    }

    fn read_quick_list_2(obj: &mut ListObject, reader: &mut RdbReader) -> anyhow::Result<()> {
        let size = reader.read_length()?;

        for _ in 0..size {
            let container = reader.read_length()?;
            match container {
                QUICKLIST_NODE_CONTAINER_PLAIN => {
                    let ele = reader.read_string()?;
                    obj.elements.push(ele);
                }

                QUICKLIST_NODE_CONTAINER_PACKED => {
                    let listpack_elements = reader.read_list_pack()?;
                    obj.elements.extend(listpack_elements);
                }

                _ => {
                    bail! {Error::RedisRdbError(format!(
                        "unknown quicklist container {}",
                        container
                    ))}
                }
            }
        }
        Ok(())
    }
}
