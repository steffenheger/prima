//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.14

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "assignment")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub departure: DateTime,
    pub arrival: DateTime,
    pub vehicle: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::event::Entity")]
    Event,
    #[sea_orm(
        belongs_to = "super::vehicle::Entity",
        from = "Column::Vehicle",
        to = "super::vehicle::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Vehicle,
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Event.def()
    }
}

impl Related<super::vehicle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicle.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
