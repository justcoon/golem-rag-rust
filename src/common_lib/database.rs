use anyhow::Result;
use golem_rust::Schema;
use serde::{Deserialize, Serialize};

// Re-export Golem RDBMS types for convenience
pub use golem_rust::bindings::golem::rdbms::postgres::{
    DbColumn as PostgresDbColumn, DbColumnType as PostgresDbColumnType,
    DbConnection as PostgresDbConnection, DbResult as PostgresDbResult, DbRow as PostgresDbRow,
    DbTransaction as PostgresDbTransaction, DbValue as PostgresDbValue,
    LazyDbValue as PostgresLazyDbValue,
};

/// A wrapper for types that should be encoded/decoded from JSON/JSONB
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Json<T>(pub T);

pub use decode::{DbResultDecoder, DbRowDecoder, DbValueDecoder, Single};
pub use encode::{DbParamsEncoder, DbValueEncoder};

pub mod decode {
    use super::*;

    pub trait DbValueDecoder: Sized {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self>;
    }

    impl<T: serde::de::DeserializeOwned> DbValueDecoder for Json<T> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Jsonb(s) | PostgresDbValue::Json(s) => serde_json::from_str(s)
                    .map(Json)
                    .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e)),
                _ => Err(anyhow::anyhow!("Expected Jsonb or Json, got {:?}", value)),
            }
        }
    }

    /// A wrapper for single-column results
    #[derive(Debug, Clone)]
    pub struct Single<T>(pub T);

    impl<T: DbValueDecoder> DbRowDecoder for Single<T> {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let value = row
                .values
                .first()
                .ok_or_else(|| anyhow::anyhow!("Row is empty"))?;
            T::decode(value).map(Single)
        }
    }

    // Tuple implementations
    impl<T1: DbValueDecoder, T2: DbValueDecoder> DbRowDecoder for (T1, T2) {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let v1 = T1::decode(
                row.values
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("Missing column 0"))?,
            )?;
            let v2 = T2::decode(
                row.values
                    .get(1)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 1"))?,
            )?;
            Ok((v1, v2))
        }
    }

    impl<T1: DbValueDecoder, T2: DbValueDecoder, T3: DbValueDecoder> DbRowDecoder for (T1, T2, T3) {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let v1 = T1::decode(
                row.values
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("Missing column 0"))?,
            )?;
            let v2 = T2::decode(
                row.values
                    .get(1)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 1"))?,
            )?;
            let v3 = T3::decode(
                row.values
                    .get(2)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 2"))?,
            )?;
            Ok((v1, v2, v3))
        }
    }

    impl<T1: DbValueDecoder, T2: DbValueDecoder, T3: DbValueDecoder, T4: DbValueDecoder>
        DbRowDecoder for (T1, T2, T3, T4)
    {
        fn decode_row(row: &PostgresDbRow, _columns: &[PostgresDbColumn]) -> anyhow::Result<Self> {
            let v1 = T1::decode(
                row.values
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("Missing column 0"))?,
            )?;
            let v2 = T2::decode(
                row.values
                    .get(1)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 1"))?,
            )?;
            let v3 = T3::decode(
                row.values
                    .get(2)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 2"))?,
            )?;
            let v4 = T4::decode(
                row.values
                    .get(3)
                    .ok_or_else(|| anyhow::anyhow!("Missing column 3"))?,
            )?;
            Ok((v1, v2, v3, v4))
        }
    }

    #[macro_export]
    macro_rules! db_value_decoder_json {
        ($t:ty) => {
            impl $crate::common_lib::database::decode::DbValueDecoder for $t {
                fn decode(
                    value: &$crate::common_lib::database::PostgresDbValue,
                ) -> anyhow::Result<Self> {
                    match value {
                        $crate::common_lib::database::PostgresDbValue::Jsonb(s)
                        | $crate::common_lib::database::PostgresDbValue::Json(s) => {
                            serde_json::from_str(s).map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to parse JSON for {}: {}",
                                    stringify!($t),
                                    e
                                )
                            })
                        }
                        _ => Err(anyhow::anyhow!(
                            "Expected Jsonb or Json for {}, got {:?}",
                            stringify!($t),
                            value
                        )),
                    }
                }
            }
        };
    }

    pub trait DbRowDecoder: Sized {
        fn decode_row(row: &PostgresDbRow, columns: &[PostgresDbColumn]) -> anyhow::Result<Self>;

        fn find_column_index(columns: &[PostgresDbColumn], name: &str) -> anyhow::Result<usize> {
            columns
                .iter()
                .position(|c| c.name == name)
                .ok_or_else(|| anyhow::anyhow!("Column {} not found", name))
        }

        fn decode_field<T: DbValueDecoder>(
            row: &PostgresDbRow,
            idx: usize,
            field_name: &str,
        ) -> anyhow::Result<T> {
            let value = row
                .values
                .get(idx)
                .ok_or_else(|| anyhow::anyhow!("Field index {} out of bounds for row", idx))?;
            DbValueDecoder::decode(value)
                .map_err(|e| anyhow::anyhow!("Error decoding field '{}': {}", field_name, e))
        }
    }

    pub trait DbResultDecoder: Sized {
        fn decode_result(result: PostgresDbResult) -> anyhow::Result<Vec<Self>>;
    }

    impl<T: DbRowDecoder> DbResultDecoder for T {
        fn decode_result(result: PostgresDbResult) -> anyhow::Result<Vec<Self>> {
            result
                .rows
                .iter()
                .map(|row| T::decode_row(row, &result.columns))
                .collect()
        }
    }

    #[macro_export]
    macro_rules! db_row_decoder {
        ($struct_name:ident { $($field:ident),* $(,)? }) => {
            impl $crate::common_lib::database::decode::DbRowDecoder for $struct_name {
                fn decode_row(
                    row: &$crate::common_lib::database::PostgresDbRow,
                    columns: &[$crate::common_lib::database::PostgresDbColumn],
                ) -> anyhow::Result<Self> {
                    let find_idx = |name: &str| {
                        <Self as $crate::common_lib::database::decode::DbRowDecoder>::find_column_index(columns, name)
                    };

                    Ok(Self {
                        $(
                            $field: {
                                let idx = find_idx(stringify!($field))?;
                                <Self as $crate::common_lib::database::decode::DbRowDecoder>::decode_field(row, idx, stringify!($field))?
                            },
                        )*
                    })
                }
            }
        };
    }

    // Implementations for common types
    impl DbValueDecoder for String {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Text(s) => Ok(s.clone()),
                PostgresDbValue::Varchar(s) => Ok(s.clone()),
                _ => Err(anyhow::anyhow!("Expected Text or Varchar, got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for bool {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Boolean(b) => Ok(*b),
                _ => Err(anyhow::anyhow!("Expected Boolean, got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for i64 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int8(i) => Ok(*i),
                PostgresDbValue::Int4(i) => Ok(*i as i64),
                PostgresDbValue::Int2(i) => Ok(*i as i64),
                _ => Err(anyhow::anyhow!(
                    "Expected Int8, Int4 or Int2 (for i64), got {:?}",
                    value
                )),
            }
        }
    }

    impl DbValueDecoder for i32 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int4(i) => Ok(*i),
                PostgresDbValue::Int2(i) => Ok(*i as i32),
                _ => Err(anyhow::anyhow!(
                    "Expected Int4 or Int2 (for i32), got {:?}",
                    value
                )),
            }
        }
    }

    impl DbValueDecoder for u64 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int8(i) => Ok(*i as u64),
                PostgresDbValue::Int4(i) => Ok(*i as u64),
                _ => Err(anyhow::anyhow!(
                    "Expected Int8 or Int4 (for u64), got {:?}",
                    value
                )),
            }
        }
    }

    impl DbValueDecoder for f32 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Float4(f) => Ok(*f),
                PostgresDbValue::Float8(f) => Ok(*f as f32),
                _ => Err(anyhow::anyhow!(
                    "Expected Float4 or Float8 (for f32), got {:?}",
                    value
                )),
            }
        }
    }

    impl DbValueDecoder for f64 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Float8(f) => Ok(*f),
                PostgresDbValue::Float4(f) => Ok(*f as f64),
                _ => Err(anyhow::anyhow!(
                    "Expected Float8 or Float4 (for f64), got {:?}",
                    value
                )),
            }
        }
    }

    impl DbValueDecoder for i16 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Int2(i) => Ok(*i),
                _ => Err(anyhow::anyhow!("Expected Int2, got {:?}", value)),
            }
        }
    }

    impl DbValueDecoder for u32 {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Oid(o) => Ok(*o),
                PostgresDbValue::Int4(i) => Ok(*i as u32),
                _ => Err(anyhow::anyhow!(
                    "Expected Oid or Int4 (for u32), got {:?}",
                    value
                )),
            }
        }
    }

    impl DbValueDecoder for Vec<u8> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Bytea(b) => Ok(b.clone()),
                _ => Err(anyhow::anyhow!("Expected Bytea, got {:?}", value)),
            }
        }
    }

    impl<T: DbValueDecoder> DbValueDecoder for Option<T> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Null => Ok(None),
                _ => T::decode(value).map(Some),
            }
        }
    }

    impl<T: DbValueDecoder> DbValueDecoder for Vec<T> {
        fn decode(value: &PostgresDbValue) -> anyhow::Result<Self> {
            match value {
                PostgresDbValue::Array(vals) => vals
                    .iter()
                    .map(|lazy| T::decode(&lazy.get()))
                    .collect::<anyhow::Result<Vec<_>>>(),
                _ => Err(anyhow::anyhow!("Expected Array, got {:?}", value)),
            }
        }
    }
}

pub mod encode {
    use super::*;

    pub trait DbValueEncoder {
        fn encode(self) -> PostgresDbValue;
    }

    #[macro_export]
    macro_rules! db_value_encoder_json {
        ($t:ty) => {
            impl $crate::common_lib::database::encode::DbValueEncoder for $t {
                fn encode(self) -> $crate::common_lib::database::PostgresDbValue {
                    $crate::common_lib::database::PostgresDbValue::Jsonb(
                        serde_json::to_string(&self).unwrap_or_else(|_| "null".to_string()),
                    )
                }
            }

            impl $crate::common_lib::database::encode::DbValueEncoder for &$t {
                fn encode(self) -> $crate::common_lib::database::PostgresDbValue {
                    $crate::common_lib::database::PostgresDbValue::Jsonb(
                        serde_json::to_string(self).unwrap_or_else(|_| "null".to_string()),
                    )
                }
            }
        };
    }

    pub trait DbParamsEncoder {
        fn encode_params(self) -> Vec<PostgresDbValue>;
    }

    impl<T: serde::Serialize> DbValueEncoder for Json<T> {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Jsonb(
                serde_json::to_string(&self.0).unwrap_or_else(|_| "null".to_string()),
            )
        }
    }

    impl DbValueEncoder for &String {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Text(self.clone())
        }
    }

    impl DbValueEncoder for String {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Text(self)
        }
    }

    impl DbValueEncoder for &str {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Text(self.to_string())
        }
    }

    impl DbValueEncoder for bool {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Boolean(self)
        }
    }

    impl DbValueEncoder for &bool {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Boolean(*self)
        }
    }

    impl DbValueEncoder for i64 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int8(self)
        }
    }

    impl DbValueEncoder for &i64 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int8(*self)
        }
    }

    impl DbValueEncoder for i32 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int4(self)
        }
    }

    impl DbValueEncoder for &i32 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int4(*self)
        }
    }

    impl DbValueEncoder for u64 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int8(self as i64)
        }
    }

    impl DbValueEncoder for &u64 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int8(*self as i64)
        }
    }

    impl DbValueEncoder for f32 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Float4(self)
        }
    }

    impl DbValueEncoder for &f32 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Float4(*self)
        }
    }

    impl DbValueEncoder for f64 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Float8(self)
        }
    }

    impl DbValueEncoder for &f64 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Float8(*self)
        }
    }

    impl DbValueEncoder for i16 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int2(self)
        }
    }

    impl DbValueEncoder for &i16 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int2(*self)
        }
    }

    impl DbValueEncoder for u32 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int4(self as i32)
        }
    }

    impl DbValueEncoder for &u32 {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Int4(*self as i32)
        }
    }

    impl DbValueEncoder for Vec<u8> {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Bytea(self)
        }
    }

    impl DbValueEncoder for &Vec<u8> {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Bytea(self.clone())
        }
    }

    impl<T: DbValueEncoder> DbValueEncoder for Option<T> {
        fn encode(self) -> PostgresDbValue {
            match self {
                Some(v) => v.encode(),
                None => PostgresDbValue::Null,
            }
        }
    }

    impl<T: DbValueEncoder + Clone> DbValueEncoder for &Option<T> {
        fn encode(self) -> PostgresDbValue {
            match self {
                Some(v) => v.clone().encode(),
                None => PostgresDbValue::Null,
            }
        }
    }

    impl<T: DbValueEncoder> DbValueEncoder for Vec<T> {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Array(
                self.into_iter()
                    .map(|v| PostgresLazyDbValue::new(v.encode()))
                    .collect(),
            )
        }
    }

    impl<T: DbValueEncoder + Clone> DbValueEncoder for &Vec<T> {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Array(
                self.iter()
                    .map(|v| PostgresLazyDbValue::new(v.clone().encode()))
                    .collect(),
            )
        }
    }

    impl<T: DbValueEncoder + Clone> DbValueEncoder for &[T] {
        fn encode(self) -> PostgresDbValue {
            PostgresDbValue::Array(
                self.iter()
                    .map(|v| PostgresLazyDbValue::new(v.clone().encode()))
                    .collect(),
            )
        }
    }

    impl DbValueEncoder for PostgresDbValue {
        fn encode(self) -> PostgresDbValue {
            self
        }
    }

    impl<T1: DbValueEncoder> DbParamsEncoder for (T1,) {
        fn encode_params(self) -> Vec<PostgresDbValue> {
            vec![self.0.encode()]
        }
    }

    impl<T1: DbValueEncoder, T2: DbValueEncoder> DbParamsEncoder for (T1, T2) {
        fn encode_params(self) -> Vec<PostgresDbValue> {
            vec![self.0.encode(), self.1.encode()]
        }
    }

    impl<T1: DbValueEncoder, T2: DbValueEncoder, T3: DbValueEncoder> DbParamsEncoder for (T1, T2, T3) {
        fn encode_params(self) -> Vec<PostgresDbValue> {
            vec![self.0.encode(), self.1.encode(), self.2.encode()]
        }
    }

    impl DbParamsEncoder for Vec<PostgresDbValue> {
        fn encode_params(self) -> Vec<PostgresDbValue> {
            self
        }
    }
}

#[macro_export]
macro_rules! encode_params {
    ($($val:expr),* $(,)?) => {
        vec![
            $(
                $crate::common_lib::database::encode::DbValueEncoder::encode($val),
            )*
        ]
    };
}

#[derive(Clone, Debug, Schema, Serialize, Deserialize)]
pub struct PostgresDbConfig {
    pub host: String,
    pub db: String,
    pub user: String,
    pub password: String,
    pub port: String,
}

impl PostgresDbConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: std::env::var("POSTGRES_HOST")
                .map_err(|_| anyhow::anyhow!("POSTGRES_HOST environment variable not set"))?,
            db: std::env::var("POSTGRES_DB")
                .map_err(|_| anyhow::anyhow!("POSTGRES_DB environment variable not set"))?,
            user: std::env::var("POSTGRES_USER")
                .map_err(|_| anyhow::anyhow!("POSTGRES_USER environment variable not set"))?,
            password: std::env::var("POSTGRES_PASSWORD")
                .map_err(|_| anyhow::anyhow!("POSTGRES_PASSWORD environment variable not set"))?,
            port: std::env::var("POSTGRES_PORT")
                .map_err(|_| anyhow::anyhow!("POSTGRES_PORT environment variable not set"))?,
        })
    }

    pub fn db_url(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.db
        )
    }
}

pub struct DatabaseHelper {
    pub connection: PostgresDbConnection,
}

impl DatabaseHelper {
    pub fn new(url: &str) -> Result<Self> {
        let connection = PostgresDbConnection::open(url)?;
        Ok(Self { connection })
    }

    pub fn from_env() -> Result<Self> {
        let config = PostgresDbConfig::from_env()?;
        Self::new(&config.db_url())
    }

    /// Execute a function within a database transaction
    ///
    /// # Arguments
    /// * `f` - A function that takes a transaction reference and returns a Result
    ///
    /// # Returns
    /// The result of the function, with automatic commit/rollback handling
    pub fn transactional<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&PostgresDbTransaction) -> Result<R>,
    {
        let transaction = self.connection.begin_transaction()?;

        match f(&transaction) {
            Ok(result) => {
                transaction.commit()?;
                Ok(result)
            }
            Err(e) => {
                if let Err(rollback_err) = transaction.rollback() {
                    log::error!("Failed to rollback transaction: {:?}", rollback_err);
                }
                Err(e)
            }
        }
    }
}
