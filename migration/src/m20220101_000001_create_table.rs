use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .create_table(
                Table::create()
                    .table(Hashes::Table)
                    .if_not_exists()
                    .col(pk_auto(Hashes::Id))
                    .col(integer(Hashes::ChatId))
                    .col(integer(Hashes::MessageId))
                    .col(string(Hashes::Filename))
                    .col(string(Hashes::FileId))
                    .col(
                        string(Hashes::Orientation).check(Expr::col(Hashes::Orientation).is_in([
                            "portrait",
                            "landscape",
                            "square",
                        ])),
                    )
                    .col(string(Hashes::Base64Hash))
                    .col(integer(Hashes::CreatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("unique-aspect-for-filename")
                    .table(Hashes::Table)
                    .col(Hashes::Filename)
                    .col(Hashes::Orientation)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Hashes::Table).to_owned())
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
}
