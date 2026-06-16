pub mod builder;
pub mod flavours;
pub mod memory;
pub mod models;

// Re-exports
pub use builder::MangoQueryBuilder;
#[cfg(feature = "dynamodb")]
pub use flavours::dynamodb::{DynamoDBCompiler, DynamoDBConfig, DynamoDBFilterOutput};
pub use flavours::postgresql::{
    PostgreSQLCompiler, PostgreSQLConfig, PostgreSQLFilterOutput, PostgresColumnConfig,
    PostgresJoinConfig,
};
pub use flavours::types::FlavourCompiler;
pub use memory::{InMemoryFilter, InMemoryFilterOptions, InMemoryFilterResult};
pub use models::{MangoQuery, SortRule, UseIndex};
