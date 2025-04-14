
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .alter_table(
                Table::alter()
                    .table(Hashes::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("media_group_id")).string().null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Hashes::Table)
                    .drop_column(Alias::new("media_group_id"))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Hashes {
    Table,
    Id,
    ChatId,
    MessageId,
    Filename,
    FileId,
    Orientation,
    Base64Hash,
    CreatedAt,
    MediaGroupId,
}
