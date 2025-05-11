use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Votings::Table)
                    .if_not_exists()
                    .col(pk_auto(Votings::Id))
                    .col(integer(Votings::ChatId))
                    .col(integer(Votings::MessageId))
                    .col(integer(Votings::OriginalMessageId))
                    .col(text(Votings::VotingType)).check(Expr::col(Votings::VotingType).is_in(["nondupes", "ignore"]))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Votes::Table)
                    .if_not_exists()
                    .col(pk_auto(Votes::Id))
                    .col(integer(Votes::VotingId))
                    .col(integer(Votes::VoteType))
                    .check(Expr::col(Votes::VoteType).is_in([-1, 1]))
                    .col(integer(Votes::UserId))
                    .col(string(Votes::Username))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Votes::Table, Votes::VotingId)
                            .to(Votings::Table, Votings::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Votes::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Votings::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Votings {
    Table,
    Id,
    ChatId,
    MessageId,
    OriginalMessageId,
    VotingType,
}

#[derive(DeriveIden)]
enum Votes {
    Table,
    Id,
    VotingId,
    VoteType,
    UserId,
    Username,
}
