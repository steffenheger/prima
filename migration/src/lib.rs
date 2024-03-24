pub use sea_orm_migration::prelude::*;

mod m20220101_000001_init;
mod m20240209_145121_user_pwd_email;
mod m20240222_160326_user_active_flag;
mod m20240225_091835_mvp;
mod m20240304_210904_vehicle_specifics_table;
mod m20240308_121823_store_adress;
mod m20240312_090704_assignment_table;
mod m20240312_152829_concrete_vehicles_replace_capacities;
mod m20240322_232453_delete_redundant_cols;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_init::Migration),
            Box::new(m20240209_145121_user_pwd_email::Migration),
            Box::new(m20240222_160326_user_active_flag::Migration),
            Box::new(m20240225_091835_mvp::Migration),
            Box::new(m20240304_210904_vehicle_specifics_table::Migration),
            Box::new(m20240308_121823_store_adress::Migration),
            Box::new(m20240312_090704_assignment_table::Migration),
            Box::new(m20240312_152829_concrete_vehicles_replace_capacities::Migration),
            Box::new(m20240322_232453_delete_redundant_cols::Migration),
        ]
    }
}
