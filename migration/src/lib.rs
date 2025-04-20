pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20250413_212102_add_mediagroup;
mod m20250419_183421_create_voting;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20250413_212102_add_mediagroup::Migration),
            Box::new(m20250419_183421_create_voting::Migration),
        ]
    }
}
