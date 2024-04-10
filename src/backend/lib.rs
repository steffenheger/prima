use crate::backend::interval::Interval;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use hyper::StatusCode;
use std::collections::HashMap;

#[async_trait]
pub trait PrimaTour {
    async fn get_vehicle_license_plate(&self) -> &str;
}

#[async_trait]
pub trait PrimaEvent {
    async fn get_id(&self) -> i32;
    async fn get_customer_name(&self) -> &str;
    async fn get_vehicle_license_plate(&self) -> &str;
    async fn get_lat(&self) -> f32;
    async fn get_lng(&self) -> f32;
    async fn get_address_id(&self) -> i32;
}

#[async_trait]
pub trait PrimaVehicle {
    async fn get_id(&self) -> i32;
    async fn get_license_plate(&self) -> &str;
    async fn get_company_id(&self) -> i32;
    async fn get_tours(&self) -> Vec<Box<&dyn PrimaTour>>;
}

#[async_trait]
pub trait PrimaUser {
    async fn get_id(&self) -> i32;
    async fn is_driver(&self) -> bool;
    async fn is_disponent(&self) -> bool;
    async fn is_admin(&self) -> bool;
    async fn get_company_id(&self) -> Option<bool>;
}

#[async_trait]
pub trait PrimaVehicleSpecifics {
    async fn get_seats(&self) -> i32;
    async fn get_wheelchairs(&self) -> i32;
    async fn get_storage_space(&self) -> i32;
}

#[async_trait]
pub trait PrimaCompany {
    async fn get_id(&self) -> i32;
    async fn get_name(&self) -> &str;
}

#[async_trait]
pub trait PrimaAvailability {
    async fn get_interval(&self) -> &Interval;
}

#[async_trait]
pub trait PrimaData: Send + Sync {
    async fn read_data_from_db(&mut self);

    async fn create_vehicle(
        &mut self,
        license_plate: &String,
        company: i32,
    ) -> StatusCode;

    async fn create_user(
        &mut self,
        name: String,
        is_driver: bool,
        is_disponent: bool,
        company: Option<i32>,
        is_admin: bool,
        email: String,
        password: Option<String>,
        salt: String,
        o_auth_id: Option<String>,
        o_auth_provider: Option<String>,
    ) -> StatusCode;

    async fn create_availability(
        &mut self,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
        vehicle: i32,
    ) -> StatusCode;

    async fn create_zone(
        &mut self,
        name: String,
        area_str: String,
    ) -> StatusCode;

    async fn create_company(
        &mut self,
        name: String,
        zone: i32,
        email: String,
        lat: f32,
        lng: f32,
    ) -> StatusCode;

    async fn remove_availability(
        &mut self,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
        vehicle_id: i32,
    ) -> StatusCode;

    async fn change_vehicle_for_tour(
        &mut self,
        tour_id: i32,
        new_vehicle_id: i32,
    ) -> StatusCode;

    async fn get_company(
        &self,
        company_id: i32,
    ) -> Result<Box<&dyn PrimaCompany>, StatusCode>;

    async fn get_tours(
        &self,
        vehicle_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<Vec<Box<&dyn PrimaTour>>, StatusCode>;

    async fn get_vehicles(
        &self,
        company_id: i32,
    ) -> Result<Vec<Box<&dyn PrimaVehicle>>, StatusCode>;

    async fn get_events_for_vehicle(
        &self,
        vehicle_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<Vec<Box<&dyn PrimaEvent>>, StatusCode>;

    async fn get_events_for_user(
        &self,
        user_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<Vec<Box<&dyn PrimaEvent>>, StatusCode>;

    async fn get_events_for_tour(
        &self,
        tour_id: i32,
    ) -> Result<Vec<Box<&dyn PrimaEvent>>, StatusCode>;

    async fn get_idle_vehicles(
        &self,
        company_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<Vec<Box<&dyn PrimaVehicle>>, StatusCode>;

    async fn is_vehicle_idle(
        &self,
        vehicle_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<bool, StatusCode>;

    async fn get_company_conflicts(
        &self,
        company_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<HashMap<i32, Vec<Box<&dyn PrimaTour>>>, StatusCode>;

    async fn get_vehicle_conflicts(
        &self,
        vehicle_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<Vec<Box<&dyn PrimaTour>>, StatusCode>;

    async fn get_tour_conflicts(
        &self,
        event_id: i32,
        company_id: Option<i32>,
    ) -> Result<Vec<Box<&dyn PrimaTour>>, StatusCode>;
}
