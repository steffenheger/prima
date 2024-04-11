use crate::{
    backend::{lib::{PrimaData, PrimaEvent, PrimaTour, PrimaUser, PrimaVehicle, PrimaCompany},
    interval::Interval},
    constants::constants::{BEELINE_KMH, KM_PRICE, MINUTE_PRICE},
    entities::{
        tour, availability, company, event, request,
        prelude::{
            Tour, Availability, Company, Event, User, Vehicle, Zone, Request
        },
        user, vehicle, zone,
    },
    error,
    osrm::{
        Coordinate,
        Dir::{Backward, Forward},
        DistTime, OSRM,
    },
    StatusCode,
};
use async_trait::async_trait;
use sea_orm::DbConn;
use super::geo_from_str::multi_polygon_from_str;
use ::anyhow::Result;
use chrono::{Duration, NaiveDateTime, NaiveDate, Utc};
use geo::{prelude::*, MultiPolygon, Point};
use itertools::Itertools;
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Hash)]
enum TourConcatenationCase {
    Prepend{ vehicle_id: i32, successor_tour_id: i32, successor_event_id: i32},
    Append{vehicle_id: i32, predecessor_tour_id: i32, predecessor_event_id: i32},
    NewTour{company_id: i32},
    Insert{vehicle_id: i32,  predecessor_tour_id: i32, successor_tour_id: i32, predecessor_event_id: i32, successor_event_id: i32}
}

fn is_user_role_valid(is_driver: bool, is_disponent: bool, is_admin: bool, company_id: Option<i32>) -> bool{
    match company_id {
        None => if is_driver || is_disponent {
            return false;
        },
        Some(_) => if !is_driver && !is_disponent {
            return false;
        },
    }
    if is_admin && (is_driver || is_disponent) {
        return false;
    }
    true
}

fn id_to_vec_pos(id: i32) -> usize {
    assert!(id>=1);
    (id - 1) as usize
}

fn seconds_to_minutes(seconds: i32) -> i32 {
    assert!(seconds>=0);
    seconds / 60
}

fn meter_to_km(m: i32) -> i32 {
    assert!(m>=0);
    m / 1000
}

fn meter_to_km_f(m: f64) -> f64 {
    assert!(m>=0.0);// TODO make sure this check can't produce errors because of rounding
    m / 1000.0
}

fn hrs_to_minutes(h: f64) -> i64 {
    assert!(h>=0.0);
    (h * 60.0) as i64
}

fn is_valid(interval: &Interval) ->bool {
    interval.start_time >= Utc::now().naive_utc() && interval.end_time <= NaiveDate::from_ymd_opt(10000, 1, 1)
    .unwrap()
    .and_hms_opt(0, 0, 0)
    .unwrap()
}

#[derive(Debug, Clone, PartialEq)]
#[readonly::make]
pub struct TourData {
    id: i32,
    departure: NaiveDateTime,                       //departure from taxi central
    arrival: NaiveDateTime,                         //arrival at taxi central
    vehicle: i32,
    events: Vec<EventData>,
}

#[async_trait]
impl PrimaTour for TourData {
    async fn get_vehicle_license_plate(&self) -> &str {
        ""
    }
}

impl TourData {
    fn print(
        &self,
        indent: &str,
    ) {
        println!(
            "{}id: {}, departure: {}, arrival: {}, vehicle: {}\n",
            indent, self.id, self.departure, self.arrival, self.vehicle
        );
    }
    
    fn overlaps(
        &self,
        interval: &Interval,
    ) -> bool {
        interval.overlaps(&Interval::new(self.departure, self.arrival))
    }

    fn any_tour_event_overlaps(//TODO
        &self,
        interval: &Interval,
    ) -> bool {
        interval.overlaps(&Interval::new(self.departure, self.arrival))
    }

    fn get_first_event(&self)->Option<&EventData>{
        self.events.iter().min_by_key(|event|event.scheduled_time)
    }

    fn get_last_event(&self)->Option<&EventData>{
        self.events.iter().max_by_key(|event|event.scheduled_time)
    } 
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[readonly::make]
pub struct AvailabilityData {
    id: i32,
    interval: Interval,
}

#[derive(PartialEq, Clone)]
#[readonly::make]
pub struct UserData {
    id: i32,
    name: String,
    is_driver: bool,
    is_admin: bool,
    email: String,
    password: Option<String>,
    salt: String,
    o_auth_id: Option<String>,
    o_auth_provider: Option<String>,
}

#[async_trait]
impl PrimaUser for UserData {
    async fn get_id(&self) -> i32{
        self.id
    }

    async fn is_driver(&self) -> bool{
        self.is_driver
    }

    async fn is_disponent(&self) -> bool{
        false // TODO
    }

    async fn is_admin(&self) -> bool{
        self.is_admin
    }

    async fn get_company_id(&self) -> Option<bool>{
        None // TODO
    }
}

#[derive(Debug, Clone, PartialEq)]
#[readonly::make]
pub struct VehicleData {
    id: i32,
    license_plate: String,
    company: i32,
    seats: i32,
    wheelchair_capacity: i32,
    storage_space: i32,
    availability: HashMap<i32, AvailabilityData>,
    tours: Vec<TourData>,
}

impl VehicleData{
    fn fulfills_requirements(&self, passengers: i32, wheelchairs: i32, luggage: i32) -> bool{
        passengers<4
    }
}

#[async_trait]
impl PrimaVehicle for VehicleData {
    async fn get_id(&self) -> i32{
        self.id
    }

    async fn get_license_plate(&self) -> &str{
        &self.license_plate
    }

    async fn get_company_id(&self) -> i32 {
        self.company
    }

    async fn get_tours(&self) -> Vec<Box<&dyn PrimaTour>> {
        self.tours.iter().map(|tour| Box::new(tour as &dyn PrimaTour)).collect_vec()
    }
}

impl VehicleData {
    fn print(&self) {
        println!(
            "id: {}, license: {}, company: {}, seats: {}, wheelchair_capacity: {}, storage_space: {}",
            self.id, self.license_plate, self.company, self.seats, self.wheelchair_capacity, self.storage_space
        );
    }
    fn new() -> Self {
        Self {
            id: -1,
            license_plate: "".to_string(),
            company: -1,
            seats: -1,
            wheelchair_capacity: -1,
            storage_space: -1,
            availability: HashMap::new(),
            tours: Vec::new(),
        }
    }
    async fn add_availability(
        &mut self,
        db_conn: &DbConn,
        new_interval: &mut Interval,
        id_or_none: Option<i32>, //None->insert availability into db, this yields the id->create availability in data with this id.  Some->create in data with given id, nothing to do in db
    ) -> StatusCode {
        let mut mark_delete: Vec<i32> = Vec::new();
        for (id, existing) in self.availability.iter() {
            if !existing.interval.overlaps(new_interval) {
                if existing.interval.touches(new_interval){
                    mark_delete.push(*id);
                    new_interval.merge(&existing.interval);
                }
                continue;
            }
            if existing.interval.contains(new_interval) {
                return StatusCode::CREATED;
            }
            if new_interval.contains(&existing.interval) {
                mark_delete.push(*id);
            }
            if new_interval.overlaps(&existing.interval) {
                mark_delete.push(*id);
                new_interval.merge(&existing.interval);
            }
        }
        for to_delete in mark_delete {
            match Availability::delete_by_id(self.availability[&to_delete].id)
                .exec(db_conn)
                .await
            {
                Ok(_) => {
                    self.availability.remove(&to_delete);
                }
                Err(e) => {
                    error!("Error deleting interval: {e:?}");
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            }
        }
        let id = match id_or_none {
            Some(i) => i,
            None => match Availability::insert(availability::ActiveModel {
                id: ActiveValue::NotSet,
                start_time: ActiveValue::Set(new_interval.start_time),
                end_time: ActiveValue::Set(new_interval.end_time),
                vehicle: ActiveValue::Set(self.id),
            })
            .exec(db_conn)
            .await
            {
                Ok(result) => result.last_insert_id,
                Err(e) => {
                    error!("Error creating availability in db: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            },
        };
        match self.availability.insert(
            id,
            AvailabilityData {
                id,
                interval: *new_interval,
            },
        ) {
            None => StatusCode::CREATED,
            Some(_) => {
                error!("Key already existed in availability");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[readonly::make]
pub struct EventData {
    id: i32,
    coordinates: Point,
    scheduled_time: NaiveDateTime,
    communicated_time: NaiveDateTime,
    customer: i32,
    tour: i32,
    passengers: i32,
    wheelchairs: i32,
    luggage: i32,
    request_id: i32,
    is_pickup: bool,
    address_id: i32
}

#[async_trait]
impl PrimaEvent for EventData {
    async fn get_id(&self) -> i32{
        self.id
    }

    async fn get_vehicle_license_plate(&self) -> &str {
        ""
    }

    async fn get_customer_name(&self) -> &str {
        ""
    }

    async fn get_lat(&self) -> f32 {
        self.coordinates.0.x as f32
    }

    async fn get_lng(&self) -> f32 {
        self.coordinates.0.y as f32
    }
    
    async fn get_address_id(&self) -> i32 {
        self.address_id
    }
}

impl EventData {
    fn print(
        &self,
        indent: &str,
    ) {
        println!(
            "{}id: {}, scheduled_time: {}, communicated_time: {}, customer: {}, tour: {}, request_id: {}, passengers: {}, wheelchairs: {}, luggage: {}, is_pickup: {}",
            indent, self.id, self.scheduled_time, self.communicated_time, self.customer, self.tour, self.request_id, self.passengers, self.wheelchairs, self.luggage, self.is_pickup
        );
    }

    fn overlaps(&self, interval: &Interval)->bool{
        interval.overlaps(&Interval::new(self.scheduled_time, self.communicated_time,))
    }
}

#[derive(PartialEq, Clone)]
#[readonly::make]
pub struct CompanyData {
    id: i32,
    central_coordinates: Point,
    zone: i32,
    name: String,
}

#[async_trait]
impl PrimaCompany for CompanyData{
    async fn get_id(&self) -> i32 {
        self.id
    }

    async fn get_name(&self) -> &str {
        &self.name
    }
}

impl CompanyData {
    fn new() -> Self {
        Self {
            id: -1,
            central_coordinates: Point::new(0.0, 0.0),
            zone: -1,
            name: "".to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct AddressData {
    id: i32,
    zip_code: String,
    city: String,
    street: String,
    house_nr: String,
}

#[derive(PartialEq, Clone)]
#[readonly::make]
pub struct ZoneData {
    area: MultiPolygon,
    name: String,
    id: i32,
}

#[derive(Clone)]
#[readonly::make]
pub struct Data {
    users: HashMap<i32, UserData>,
    zones: Vec<ZoneData>,                         //indexed by (id-1)
    companies: Vec<CompanyData>,                  //indexed by (id-1)
    vehicles: Vec<VehicleData>,               //indexed by (id-1)
    addresses: Vec<AddressData>,
    next_request_id: i32,
    db_connection: DbConn,
}

impl PartialEq for Data {
    fn eq(&self, other: &Data) -> bool { 
        self.users == other.users 
        && self.zones == other.zones 
        && self.companies == other.companies 
        && self.vehicles == other.vehicles
        && self.next_request_id == other.next_request_id 
     }
}

#[async_trait]
impl PrimaData for Data{
        async fn handle_routing_request(
            &mut self, 
            fixed_time: NaiveDateTime,
            is_start_time_fixed: bool,
            start_lat: f32,
            start_lng: f32,
            target_lat: f32,
            target_lng: f32,
            customer: i32,
            passengers: i32,
            start_address: &String,
            target_address: &String,
            //wheelchairs: i32, luggage: i32,
        ) -> StatusCode {
            let minimum_prep_time: Duration = Duration::hours(1);
            let now: NaiveDateTime = Utc::now().naive_utc();
            if now > fixed_time {
                return StatusCode::NOT_ACCEPTABLE;
            }
            if passengers > 3 {                                                                 // TODO: change when mvp restriction is lifted
                return StatusCode::NO_CONTENT;
            }
    
            let start = Point::new(start_lat as f64, start_lng as f64);
            let target = Point::new(target_lat as f64, target_lng as f64);
            let beeline_time = Duration::minutes(hrs_to_minutes(
                meter_to_km_f(start.geodesic_distance(&target)) / BEELINE_KMH,
            ));
            let (start_time, target_time) = if is_start_time_fixed {
                (fixed_time, fixed_time + beeline_time)
            } else {
                (fixed_time - beeline_time, fixed_time)
            };
            if now + minimum_prep_time > start_time {
                return StatusCode::NO_CONTENT;
            }
    
            let beeline_interval = Interval::new(start_time, target_time);
            // Find companies and vehicles that may process the request according to their zone, and vehicle-specifics.
            // Also filter for availability and collisions with other tours (based on their first and last events and the beeline distance between start and target of the newly requested tour)
            let candidate_vehicles = self.get_viable_vehicles(&beeline_interval, passengers, &start).await;
            let mut n_companies = 0;
            let mut viable_concatenations = Vec::<TourConcatenationCase>::new();
            for (company_id, vehicles) in candidate_vehicles.iter().into_group_map_by(|vehicle|vehicle.company){
                if vehicles.iter().any(|vehicle|!vehicle.tours.iter().any(|tour|tour.overlaps(&beeline_interval))){
                    viable_concatenations.push(TourConcatenationCase::NewTour {company_id});
                    n_companies += 1;
                }
            }
            for vehicle in candidate_vehicles.iter() {
                let predecessor_tour = match vehicle.tours.iter()
                    .filter(|tour|tour.get_last_event().unwrap().scheduled_time < start_time)
                    .max_by_key(|tour|tour.get_last_event().unwrap().scheduled_time){
                    None => {
                        return StatusCode::INTERNAL_SERVER_ERROR;
                    },
                    Some(tour)=>tour,
                };
                let predecessor_event_id = predecessor_tour.get_last_event().unwrap().id;
                let successor_tour = match vehicle.tours.iter()
                    .filter(|tour|tour.get_first_event().unwrap().scheduled_time < target_time)
                    .max_by_key(|tour|tour.get_first_event().unwrap().scheduled_time){
                    None => {
                        return StatusCode::INTERNAL_SERVER_ERROR;
                    },
                    Some(tour)=>tour,
                };
                let successor_event_id = successor_tour.get_first_event().unwrap().id;
                viable_concatenations.push(TourConcatenationCase::Insert { vehicle_id: vehicle.id, predecessor_tour_id: predecessor_tour.id, successor_tour_id: successor_tour.id, predecessor_event_id, successor_event_id});
                if successor_tour.arrival < start_time{
                    viable_concatenations.push(TourConcatenationCase::Append { vehicle_id: vehicle.id, predecessor_tour_id: predecessor_tour.id, predecessor_event_id});
                }
                if predecessor_tour.departure > target_time{
                    viable_concatenations.push(TourConcatenationCase::Prepend { vehicle_id: vehicle.id, successor_tour_id: successor_tour.id, successor_event_id});
                }
            }
    
            for v in candidate_vehicles.iter() {
                println!("company: {}, vehicle: {}", v.company, v.id);
            }
            let start_c = Coordinate {
                lng: start_lat as f64,
                lat: start_lng as f64,
            };
            let target_c = Coordinate {
                lng: target_lat as f64,
                lat: target_lng as f64,
            };
            
            let mut start_many = Vec::<Coordinate>::with_capacity(n_companies+candidate_vehicles.len()+1);
            let mut pos = 0;
            for case in viable_concatenations.iter() {
                match case{
                    TourConcatenationCase::Append{vehicle_id, predecessor_tour_id, predecessor_event_id} =>(),
                    TourConcatenationCase::Prepend{vehicle_id, successor_tour_id, successor_event_id} =>(),
                    TourConcatenationCase::NewTour{company_id} => {
                        start_many[pos] = Coordinate {
                            lat: self.get_company(*company_id).central_coordinates.y(),
                            lng: self.get_company(*company_id).central_coordinates.x(),
                        };
                        pos+=1;
                    },
                    TourConcatenationCase::Insert{vehicle_id, predecessor_tour_id, successor_tour_id, predecessor_event_id, successor_event_id} => {
                        let predecessor_event_coordinates = self.get_event(*vehicle_id, *predecessor_tour_id, *predecessor_event_id).await.coordinates;
                        start_many[pos] = Coordinate {
                            lat: predecessor_event_coordinates.y(),
                            lng: predecessor_event_coordinates.x(),
                        };
                        pos+=1;
                    },
                }
            }
            // add this to get distance between start and target of the new request
            start_many[pos] = Coordinate {
                lat: target_c.lat,
                lng: target_c.lng,
            };
            let osrm = OSRM::new();
            let mut distances_to_start: Vec<DistTime> =
                match osrm.one_to_many(start_c, start_many, Backward).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!("problem with osrm: {}", e);
                        Vec::new()
                    }
                };
            let mut target_many = Vec::<Coordinate>::with_capacity(n_companies+candidate_vehicles.len());
            pos = 0;
            for case in viable_concatenations {
                match case{
                    TourConcatenationCase::Append{vehicle_id, predecessor_tour_id, predecessor_event_id} =>(),
                    TourConcatenationCase::Prepend{vehicle_id, successor_tour_id, successor_event_id} =>(),
                    TourConcatenationCase::NewTour{company_id} => {
                        target_many[pos] = Coordinate {
                            lat: self.get_company(company_id).central_coordinates.y(),
                            lng: self.get_company(company_id).central_coordinates.x(),
                        };
                        pos+=1
                    },
                    TourConcatenationCase::Insert{vehicle_id, predecessor_tour_id, successor_tour_id, predecessor_event_id, successor_event_id} => {
                        let successor_event_coordinates = self.get_event(vehicle_id, successor_tour_id, successor_event_id).await.coordinates;
                        target_many.push(Coordinate {
                            lat: successor_event_coordinates.y(),
                            lng: successor_event_coordinates.x(),
                        });
                        pos+=1;
                    },
                }
            }
            let distances_to_target: Vec<DistTime> =
                match osrm.one_to_many(target_c, target_many, Forward).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!("problem with osrm: {}", e);
                        Vec::new()
                    }
                };
    
            let travel_duration = match distances_to_start.last() {
                Some(dt) => Duration::minutes(seconds_to_minutes(dt.time as i32) as i64),
                None => {
                    error!("distances to start was empty");
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            };
            //start/target times using travel time
            let start_time = if is_start_time_fixed {
                fixed_time
            } else {
                fixed_time - travel_duration
            };
            let target_time = if is_start_time_fixed {
                fixed_time + travel_duration
            } else {
                fixed_time
            };
            if now + minimum_prep_time > start_time {
                return StatusCode::NO_CONTENT;
            }
            distances_to_start.truncate(distances_to_start.len() - 1); //remove distance from start to target
    /*
            //create all viable combinations
            let mut viable_combinations: Vec<Combination> = Vec::new();
            for (pos_in_viable, company) in viable_companies.iter().enumerate() {
                let pos_in_data = id_to_vec_pos(company.id);
                let start_dist_time = distances_to_start[pos_in_viable];
                let target_dist_time = distances_to_target[pos_in_viable];
    
                let company_start_cost = seconds_to_minutes(start_dist_time.time as i32) * MINUTE_PRICE
                    + meter_to_km(start_dist_time.dist as i32) * KM_PRICE;
    
                let company_target_cost = seconds_to_minutes(target_dist_time.time as i32)
                    * MINUTE_PRICE
                    + meter_to_km(target_dist_time.dist as i32) * KM_PRICE;
                viable_combinations.push(Combination {
                    is_start_company: true,
                    start_pos: pos_in_data,
                    is_target_company: true,
                    target_pos: pos_in_data,
                    cost: company_start_cost + company_target_cost,
                });
            }
    
            if viable_combinations.is_empty() {
                return StatusCode::NO_CONTENT;
            }
    
            viable_combinations.sort_by(|a, b| a.cost.cmp(&(b.cost)));
     */
            self.next_request_id += 1;
            self
                .insert_or_addto_tour(
                    None,
                    start_time,
                    target_time,
                    1,
                    start_address,
                    target_address,
                    start_lat,
                    start_lng,
                    start_time,
                    start_time,
                    customer,
                    1,
                    0,
                    0,
                    self.next_request_id,
                    false,
                    false,
                    target_lat,
                    target_lng,
                    target_time,
                    target_time,
                )
                .await
        } 

    async fn read_data_from_db(
        &mut self,
    ) {
        let mut zones: Vec<zone::Model> = Zone::find()
            .all(&self.db_connection)
            .await
            .expect("Error while reading from Database.");
        zones.sort_by_key(|z| z.id);
        for zone in zones.iter() {
            match multi_polygon_from_str(&zone.area) {
                Err(e) => error!("{e:?}"),
                Ok(mp) => self.zones.push(ZoneData {
                    area: mp,
                    name: zone.name.to_string(),
                    id: zone.id,
                }),
            }
        }

        let company_models: Vec<<company::Entity as sea_orm::EntityTrait>::Model> = Company::find()
            .all(&self.db_connection)
            .await
            .expect("Error while reading from Database.");
        self.companies
            .resize(company_models.len(), CompanyData::new());
        for company_model in company_models {
            self.companies[id_to_vec_pos(company_model.id)] = CompanyData {
                name: company_model.display_name,
                zone: company_model.zone,
                central_coordinates: Point::new(
                    company_model.latitude as f64,
                    company_model.longitude as f64,
                ),
                id: company_model.id,
            };
        }

        let mut vehicle_models: Vec<<vehicle::Entity as sea_orm::EntityTrait>::Model> =
            Vehicle::find()
                .all(&self.db_connection)
                .await
                .expect("Error while reading from Database.");
        self.vehicles
            .resize(vehicle_models.len(), VehicleData::new());
        vehicle_models.sort_by_key(|v| v.id);
        for vehicle in vehicle_models.iter() {
            self.vehicles[id_to_vec_pos(vehicle.id)] = VehicleData {
                id: vehicle.id,
                license_plate: vehicle.license_plate.to_string(),
                company: vehicle.company,
                seats: vehicle.seats,
                wheelchair_capacity: vehicle.wheelchair_capacity,
                storage_space: vehicle.storage_space,
                availability: HashMap::new(),
                tours: Vec::new(),
            };
        }

        let availability_models: Vec<<availability::Entity as sea_orm::EntityTrait>::Model> =
            Availability::find()
                .all(&self.db_connection)
                .await
                .expect("Error while reading from Database.");
        for availability in availability_models.iter() {
            self.vehicles[id_to_vec_pos(availability.vehicle)]
                .add_availability(
                    &self.db_connection,
                    &mut Interval::new(availability.start_time, availability.end_time),
                    Some(availability.id),
                )
                .await;
        }

        let tour_models: Vec<<tour::Entity as sea_orm::EntityTrait>::Model> =
            Tour::find()
                .all(&self.db_connection)
                .await
                .expect("Error while reading from Database.");
        for tour in tour_models {
            self.vehicles[id_to_vec_pos(tour.vehicle)].tours.push(
                TourData {
                    arrival: tour.arrival,
                    departure: tour.departure,
                    id: tour.id,
                    vehicle: tour.vehicle,
                    events: Vec::new(),
                },
            );
        }
        let event_models: Vec<<event::Entity as sea_orm::EntityTrait>::Model> = Event::find()
            .all(&self.db_connection)
            .await
            .expect("Error while reading from Database.");
        for event_m in event_models {
            let request_m: <request::Entity as sea_orm::EntityTrait>::Model = Request::find_by_id(event_m.request).one(&self.db_connection).await.expect("Error while reading from Database.").unwrap();
            let vehicle_id = self.get_tour(request_m.tour).await.unwrap().vehicle;
            self.vehicles[id_to_vec_pos(vehicle_id)].tours[id_to_vec_pos(request_m.tour)].events.push(
                EventData{
                    id: event_m.id,
                    coordinates: Point::new(event_m.latitude as f64, event_m.longitude as f64),
                    scheduled_time: event_m.scheduled_time,
                    communicated_time: event_m.communicated_time,
                    customer: request_m.customer,
                    passengers: request_m.passengers,
                    wheelchairs: request_m.wheelchairs,
                    luggage: request_m.luggage,
                    tour: request_m.tour,
                    request_id: event_m.request,
                    is_pickup: event_m.is_pickup,
                    address_id: event_m.address,
                }
            );
        }

        let user_models = User::find()
            .all(&self.db_connection)
            .await
            .expect("Error while reading from Database.");
        for user_model in user_models {
            self.users.insert(
                user_model.id,
                UserData {
                    id: user_model.id,
                    name: user_model.display_name,
                    is_driver: user_model.is_driver,
                    is_admin: user_model.is_admin,
                    email: user_model.email,
                    password: user_model.password,
                    salt: user_model.salt,
                    o_auth_id: user_model.o_auth_id,
                    o_auth_provider: user_model.o_auth_provider,
                },
            );
        }
        self.next_request_id = self.max_event_id();
    }

    async fn create_vehicle(
        &mut self,
        license_plate: &String,
        company: i32,
    ) -> StatusCode {
        if self.companies.len() < company as usize {
            return StatusCode::EXPECTATION_FAILED;
        }
        if self
            .vehicles
            .iter()
            .any(|vehicle| &vehicle.license_plate.to_string() == license_plate)
        {
            return StatusCode::CONFLICT;
        }
        let seats = 3;
        let wheelchair_capacity = 0;
        let storage_space = 0;

        match Vehicle::insert(vehicle::ActiveModel {
            id: ActiveValue::NotSet,
            company: ActiveValue::Set(company),
            license_plate: ActiveValue::Set(license_plate.to_string()),
            seats: ActiveValue::Set(seats),
            wheelchair_capacity: ActiveValue::Set(wheelchair_capacity),
            storage_space: ActiveValue::Set(storage_space),
        })
        .exec(&self.db_connection)
        .await
        {
            Ok(result) => {
                self.vehicles.push(VehicleData {
                    id: result.last_insert_id,
                    license_plate: license_plate.to_string(),
                    company,
                    seats,
                    wheelchair_capacity,
                    storage_space,
                    availability: HashMap::new(),
                    tours: Vec::new(),
                });
                StatusCode::CREATED
            }
            Err(e) => {
                error!("{e:?}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    async fn create_user(
        &mut self,
        name: &str,
        is_driver: bool,
        is_disponent: bool,
        company: Option<i32>,
        is_admin: bool,
        email: &str,
        password: Option<String>,
        salt: &str,
        o_auth_id: Option<String>,
        o_auth_provider: Option<String>,
    ) -> StatusCode {
        if self.users.iter().any(|(_, user)| user.email == *email) {
            return StatusCode::CONFLICT;
        }
        if !is_user_role_valid(is_driver, is_disponent, is_admin, company){
            return StatusCode::BAD_REQUEST;
        }
        match User::insert(user::ActiveModel {
            id: ActiveValue::NotSet,
            display_name: ActiveValue::Set(name.to_string()),
            is_driver: ActiveValue::Set(is_driver),
            is_admin: ActiveValue::Set(is_admin),
            email: ActiveValue::Set(email.to_string()),
            password: ActiveValue::Set(password.clone()),
            salt: ActiveValue::Set(salt.to_string()),
            o_auth_id: ActiveValue::Set(o_auth_id.clone()),
            o_auth_provider: ActiveValue::Set(o_auth_provider.clone()),
            company: ActiveValue::Set(company),
            is_active: ActiveValue::Set(true),
            is_disponent: ActiveValue::Set(is_disponent),
        })
        .exec(&self.db_connection)
        .await
        {
            Ok(result) => {
                let id = result.last_insert_id;
                self.users.insert(
                    id,
                    UserData {
                        id,
                        name: name.to_string(),
                        is_driver,
                        is_admin,
                        email: email.to_string(),
                        password,
                        salt: salt.to_string(),
                        o_auth_id,
                        o_auth_provider,
                    },
                );
                StatusCode::CREATED
            }
            Err(e) => {
                error!("{e:}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    async fn create_availability(
        &mut self,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
        vehicle: i32,
    ) -> StatusCode {
        let mut interval = Interval::new(start_time, end_time);
        if !is_valid(&interval) {
            return StatusCode::NOT_ACCEPTABLE;
        }
        if self.vehicles.len() < vehicle as usize {
            return StatusCode::EXPECTATION_FAILED;
        }
        self.vehicles[id_to_vec_pos(vehicle)]
            .add_availability(&self.db_connection, &mut interval, None)
            .await
    }

    async fn create_zone(
        &mut self,
        name: &str,
        area_str: &str,
    ) -> StatusCode {
        if self.zones.iter().any(|zone| zone.name == name) {
            return StatusCode::CONFLICT;
        }
        let area = match multi_polygon_from_str(area_str) {
            Err(_) => {
                return StatusCode::BAD_REQUEST;
            }
            Ok(mp) => mp,
        };
        match Zone::insert(zone::ActiveModel {
            id: ActiveValue::NotSet,
            name: ActiveValue::Set(name.to_string()),
            area: ActiveValue::Set(area_str.to_string()),
        })
        .exec(&self.db_connection)
        .await
        {
            Err(e) => {
                error!("{e:?}");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
            Ok(result) => {
                self.zones.push(ZoneData {
                    id: result.last_insert_id,
                    area,
                    name: name.to_string(),
                });
                StatusCode::CREATED
            }
        }
    }

    async fn create_company(
        &mut self,
        name: &str,
        zone: i32,
        email: &str,
        lat: f32,
        lng: f32,
    ) -> StatusCode {
        if self.zones.len() < zone as usize {
            return StatusCode::EXPECTATION_FAILED;
        }
        if self.companies.iter().any(|company| company.name == name) {
            return StatusCode::CONFLICT;
        }
        match Company::insert(company::ActiveModel {
            id: ActiveValue::NotSet,
            longitude: ActiveValue::Set(lng),
            latitude: ActiveValue::Set(lat),
            display_name: ActiveValue::Set(name.to_string()),
            zone: ActiveValue::Set(zone),
            email: ActiveValue::Set(email.to_string()),
        })
        .exec(&self.db_connection)
        .await
        {
            Ok(result) => {
                self.companies.push(CompanyData {
                    id: result.last_insert_id,
                    central_coordinates: Point::new(lat as f64, lng as f64),
                    zone,
                    name: name.to_string(),
                });
                StatusCode::CREATED
            }
            Err(e) => {
                error!("{e:?}");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
    }

    async fn remove_availability(
        &mut self,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
        vehicle_id: i32,
    ) -> StatusCode {
        match self.is_vehicle_idle(vehicle_id, start_time, end_time).await{
            Ok(idle) => if !idle{
                return StatusCode::NOT_ACCEPTABLE;
            },
            Err(e)=>return e,
        }
        let to_remove_interval = Interval::new(start_time, end_time);
        if !is_valid(&to_remove_interval) {
            return StatusCode::NOT_ACCEPTABLE;
        }
        let mut mark_delete: Vec<i32> = Vec::new();
        let mut to_insert: Option<(Interval, Interval)> = None;
        let vehicle = &mut self.vehicles[id_to_vec_pos(vehicle_id)];
        let mut touched = false;
        for (id, existing) in vehicle.availability.iter_mut() {
            if !existing.interval.overlaps(&to_remove_interval) {
                continue;
            }
            touched = true;
            if existing.interval.contains(&to_remove_interval) {
                mark_delete.push(*id);
                to_insert = Some(existing.interval.split(&to_remove_interval));
                break;
            }
            if to_remove_interval.contains(&existing.interval) {
                mark_delete.push(*id);
                continue;
            }
            if to_remove_interval.overlaps(&existing.interval) {
                existing.interval.cut(&to_remove_interval);
                let mut active_m: availability::ActiveModel =
                    match Availability::find_by_id(existing.id)
                        .one(&self.db_connection)
                        .await
                    {
                        Err(e) => {
                            error!("{e:?}");
                            return StatusCode::INTERNAL_SERVER_ERROR;
                        }
                        Ok(model_opt) => match model_opt {
                            None => return StatusCode::INTERNAL_SERVER_ERROR,
                            Some(model) => (model as availability::Model).into()
                        },
                    };
                active_m.start_time = ActiveValue::set(existing.interval.start_time);
                active_m.end_time = ActiveValue::set(existing.interval.end_time);
                match Availability::update(active_m).exec(&self.db_connection).await {
                    Ok(_) => (),
                    Err(e) => {
                        error!("Error deleting interval: {e:?}");
                        return StatusCode::INTERNAL_SERVER_ERROR;
                    }
                }
            }
        }
        if !touched {
            return StatusCode::NO_CONTENT; //no error occured but the transmitted interval did not touch any availabilites for the transmitted vehicle
        }
        for to_delete in mark_delete {
            match Availability::delete_by_id(vehicle.availability[&to_delete].id)
                .exec(&self.db_connection)
                .await
            {
                Ok(_) => {
                    vehicle.availability.remove(&to_delete);
                }
                Err(e) => {
                    error!("Error deleting interval: {e:?}");
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
            }
        }
        if let Some((left, right)) = to_insert {
            self.create_availability(left.start_time, left.end_time, vehicle_id)
            .await;
            self.create_availability(right.start_time, right.end_time, vehicle_id)
            .await;
        }
        StatusCode::OK
    }

    async fn change_vehicle_for_tour(
        &mut self,
        tour_id: i32,
        new_vehicle_id: i32,
    ) -> StatusCode {
        if self.max_vehicle_id() < new_vehicle_id  || new_vehicle_id as usize <= 0{
            return StatusCode::NOT_FOUND;
        }
        let old_vehicle_id = match self.get_tour(tour_id).await{
            Ok(tour)=> tour.vehicle,
            Err(e)=>return e,
        };
        let tour_vec_pos = self.vehicles[id_to_vec_pos(old_vehicle_id)].tours.iter().enumerate().find(|(_, tour)| tour.id == tour_id).map(|(pos, _)|pos).unwrap();
        let mut moved_tour = self.vehicles[id_to_vec_pos(old_vehicle_id)].tours.remove(tour_vec_pos);
        moved_tour.vehicle = new_vehicle_id;
        self.vehicles[id_to_vec_pos(new_vehicle_id)].tours.push(moved_tour);

        let mut active_m: tour::ActiveModel =
            match Tour::find_by_id(tour_id).one(&self.db_connection).await {
                Err(e) => {
                    error!("{e:?}");
                    return StatusCode::INTERNAL_SERVER_ERROR;
                }
                Ok(m) => match m {
                    None => return StatusCode::INTERNAL_SERVER_ERROR,
                    Some(model) => (model as tour::Model).into()
                },
            };
        active_m.vehicle = ActiveValue::Set(new_vehicle_id);
        match active_m.update(&self.db_connection).await {
            Ok(_) => (),
            Err(e) => {
                error!("{}", e);
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
        StatusCode::ACCEPTED
    }

    async fn get_company(
        &self,
        company_id: i32,
    ) -> Result<Box<&dyn PrimaCompany>, StatusCode>{
        if self.companies.len() <= company_id as usize{
            return  Err(StatusCode::NOT_FOUND);
        }
        Ok(Box::new(self.companies.iter().find(|company| company.id == company_id).unwrap() as &dyn PrimaCompany))
    }

    async fn get_tours(
        &self,
        vehicle_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<Vec<Box<&'_ dyn PrimaTour>>, StatusCode> {
        if self.max_vehicle_id() < vehicle_id  || vehicle_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        Ok(self.vehicles[id_to_vec_pos(vehicle_id)]
            .tours
            .iter()
            .filter(|tour|tour.overlaps(&Interval::new(time_frame_start,time_frame_end)))
            .map(|tour| Box::new(tour as &dyn PrimaTour))
            .collect_vec())
    }

    async fn get_events_for_vehicle(
        &self,
        vehicle_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<Vec<Box<&'_ dyn PrimaEvent>>, StatusCode> {
        if self.max_vehicle_id() < vehicle_id || vehicle_id as usize <= 0 {
            return Err(StatusCode::NOT_FOUND);
        }
        let interval = Interval::new(time_frame_start, time_frame_end);
        Ok(self
            .vehicles[id_to_vec_pos(vehicle_id)].tours
            .iter()
            .flat_map(|tour|&tour.events)
            .filter(|event| event.overlaps(&interval))
            .map(|event| Box::new(event as &'_ dyn PrimaEvent))
            .collect_vec())
    }

    async fn get_vehicles(
        &self,
        company_id: i32,
    ) -> Result<Vec<Box<&'_ dyn PrimaVehicle>>, StatusCode> {
        if self.max_company_id() < company_id  || company_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        Ok(self
            .vehicles
            .iter()
            .filter(|vehicle| {
                vehicle.company == company_id
            })
            .map(|vehicle| Box::new(vehicle as &'_ dyn PrimaVehicle))
            .collect_vec())
    }

    async fn get_events_for_user(
        &self,
        user_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<Vec<Box<&'_ dyn PrimaEvent>>, StatusCode> {
        if self.max_user_id() < user_id  || user_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        Ok(self
            .vehicles.iter().flat_map(|vehicle| 
                vehicle.tours
                    .iter()
                    .flat_map(|tour| &tour.events))
            .filter(|event| {
                event.overlaps(&Interval::new(time_frame_start,time_frame_end)) 
                && event.customer == user_id
            })
            .map(|event| Box::new(event as &dyn PrimaEvent))
            .collect_vec())
    }

    async fn get_idle_vehicles(
        &self,
        company_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<Vec<Box<&'_ dyn PrimaVehicle>>, StatusCode> {
        if self.max_company_id() < company_id  || company_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        let tour_interval = match self.get_tour(tour_id).await {
            Ok(t) => Interval::new(t.departure, t.arrival),
            Err(code) => return Err(code),
        };
        Ok(self
            .vehicles
            .iter()
            .filter(|vehicle| {
                vehicle.company == company_id
                    && !vehicle.tours
                    .iter()
                    .filter(|tour|(consider_provided_tour_conflict || tour_id!=tour.id))
                    .any(|tour| tour.overlaps(&tour_interval)
                    )
            })
            .map(|vehicle| Box::new(vehicle as &dyn PrimaVehicle))
            .collect_vec())
    }

    async fn is_vehicle_idle(
        &self,
        vehicle_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<bool, StatusCode> {
        if self.max_vehicle_id() < vehicle_id  || vehicle_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        let tour_interval = match self.get_tour(tour_id).await {
            Ok(t) => Interval::new(t.departure, t.arrival),
            Err(code) => return Err(code),
        };
        Ok(!self.vehicles[id_to_vec_pos(vehicle_id)]
            .tours
            .iter()
            .filter(|tour|(consider_provided_tour_conflict || tour_id!=tour.id))
            .any(|tour| tour.overlaps(&tour_interval)
            ))
    }

    //return vectors of conflicting tours by vehicle ids as keys
    //does not consider the provided tour_id as a conflict
    async fn get_company_conflicts(
        &self,
        company_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<HashMap<i32, Vec<Box<&'_ dyn PrimaTour>>>, StatusCode> {
        if self.max_company_id() < company_id  || company_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        let provided_tour_interval = match self.get_tour(tour_id).await {
            Ok(t) => Interval::new(t.departure, t.arrival),
            Err(code) => return Err(code),
        };

        let mut ret = HashMap::<i32, Vec<Box<&dyn PrimaTour>>>::new();
        self.vehicles
            .iter()
            .filter(|vehicle| vehicle.company == company_id)
            .for_each(|vehicle| {
                let conflicts = vehicle
                    .tours
                    .iter()
                    .filter(|tour| {     
                        (consider_provided_tour_conflict || tour_id != tour.id)
                        && tour.overlaps(&provided_tour_interval)
                    })
                    .map(|tour| Box::new(tour as &dyn PrimaTour))
                    .collect_vec();
                if !conflicts.is_empty() {
                    ret.insert(vehicle.id, conflicts);
                }
            });
        Ok(ret)
    }

    //does not consider the provided tour_id as a conflict
    async fn get_vehicle_conflicts(
        &self,
        vehicle_id: i32,
        tour_id: i32,
        consider_provided_tour_conflict: bool,
    ) -> Result<Vec<Box<&'_ dyn PrimaTour>>, StatusCode> {
        if self.max_vehicle_id() < vehicle_id  || vehicle_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        let tour_interval = match self.get_tour(tour_id).await {
            Ok(t) => Interval::new(t.departure, t.arrival),
            Err(code) => return Err(code),
        };
        Ok(self.vehicles[id_to_vec_pos(vehicle_id)]
            .tours
            .iter()
            .filter(|tour| 
                (consider_provided_tour_conflict || tour_id != tour.id)
                && tour.overlaps(&tour_interval)
            )
            .map(|tour| Box::new(tour as &dyn PrimaTour))
            .collect_vec())
    }

    async fn get_tour_conflicts(
        &self,
        event_id: i32,
        company_id: Option<i32>
        )->Result<Vec<Box<&'_ dyn PrimaTour>>,StatusCode> {
            if self.max_event_id() < event_id || event_id as usize <= 0{
                return Err(StatusCode::NOT_FOUND);
            }
            match company_id{
                None=>(),
                Some(id)=>if self.max_company_id()<id || id as usize <= 0{
                    return Err(StatusCode::NOT_FOUND);
                }
            }
            let event = match self.find_event(event_id).await{
                None=>return Err(StatusCode::NOT_FOUND),
                Some(e)=>e,
            };
            Ok(self.vehicles
                .iter()
                .filter(|vehicle|match company_id{
                    None=>true,
                    Some(id)=>vehicle.company==id
                })
                .flat_map(|vehicle|&vehicle.tours)
                .filter(|tour|tour.overlaps(&Interval::new(event.communicated_time,event.scheduled_time)))
                .map(|tour| Box::new(tour as &dyn PrimaTour))
                .collect_vec())
    }

    async fn get_events_for_tour(
        &self,
        tour_id: i32,
    ) -> Result<Vec<Box<&'_ dyn PrimaEvent>>, StatusCode> {
        if self.max_tour_id() < tour_id  || tour_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        match self.get_tour(tour_id).await{
            Err(e)=>return Err(e),
            Ok(tour)=>return Ok(tour.events
            .iter()
            .map(|event| Box::new(event as &dyn PrimaEvent))
            .collect_vec())
        };
    }
    
    async fn is_vehicle_available(
        &self,
        vehicle_id: i32,
        tour_id: i32,
    ) -> Result<bool, StatusCode> {
        if self.max_vehicle_id() < vehicle_id  || vehicle_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        let vehicle = &self.vehicles[id_to_vec_pos(vehicle_id)];
        let tour = match vehicle.tours.iter().find(|tour| tour.id == tour_id){
            Some(t) => t,
            None => return Err(StatusCode::NOT_FOUND),
        };
        let tour_interval = Interval::new(
            tour.departure,
            tour.arrival
        );
        Ok(vehicle
            .availability
            .iter()
            .any(|(_, availability)| availability.interval.contains(&tour_interval)))
    }

    async fn get_availability_intervals(
        &self,
        vehicle_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<Vec<&Interval>, StatusCode>{
        if self.max_vehicle_id() < vehicle_id  || vehicle_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        Ok(match self.vehicles.iter().find(|vehicle| vehicle.id == vehicle_id){
            Some(v) => v,
            None=> return Err(StatusCode::INTERNAL_SERVER_ERROR),
        }.availability.values().map(|availability| &availability.interval).filter(|availability_interval| Interval::new(time_frame_start, time_frame_end).overlaps(availability_interval)).collect_vec())
    }
}// end of PrimaData Trait implementation

impl Data {
    pub fn new(db_connection: &DbConn) -> Self {
        Self {
            zones: Vec::<ZoneData>::new(),
            companies: Vec::<CompanyData>::new(),
            vehicles: Vec::<VehicleData>::new(),
            users: HashMap::<i32, UserData>::new(),
            addresses: Vec::new(),
            next_request_id: 1,
            db_connection: db_connection.clone(),
        }
    }

    #[allow(dead_code)] //test/output function
    pub fn clear(&mut self) {
        self.users.clear();
        self.companies.clear();
        self.vehicles.clear();
        self.zones.clear();
        self.next_request_id = 1;
    }

    //expected to be used in fuzzing tests
    //check wether there is a vehicle for which any of the availability intervals overlap
    #[allow(dead_code)] //test/output function
    pub fn do_intervals_overlap(&self) -> bool {
        self.vehicles
            .iter()
            .map(|vehicle| &vehicle.availability)
            .any(|availability| {
                let availability_intervals = availability
                    .iter()
                    .map(|(_, availability)| availability.interval);
                availability_intervals
                    .clone()
                    .zip(availability_intervals)
                    .filter(|interval_pair| interval_pair.0 != interval_pair.1)
                    .any(|interval_pair| interval_pair.0.overlaps(&interval_pair.1))
            })
    }

    #[allow(dead_code)] //test/output function
    fn print_tours(
        &self,
        print_events: bool,
    ) {
        for tour in self.vehicles.iter().flat_map(|vehicle| &vehicle.tours) {
            tour.print("");
            if print_events {
                for event in tour.events.iter() {
                    if tour.id != event.tour {
                        continue;
                    }
                    event.print("   ");
                }
            }
        }
    }

    #[allow(dead_code)] //test/output function
    pub fn print_vehicles(&self) {
        for v in &self.vehicles {
            v.print();
        }
    }

    fn max_event_id(&self)->i32{
        self.vehicles.iter().flat_map(|vehicle|vehicle.tours.iter().flat_map(|tour|&tour.events)).map(|event|event.id).max().unwrap_or(0)
    }

    fn max_company_id(&self)->i32{
        self.companies.iter().map(|company|company.id).max().unwrap_or(0)
    }

    fn max_tour_id(&self)->i32{
        self.vehicles.iter().flat_map(|vehicle|&vehicle.tours).map(|tour|tour.id).max().unwrap_or(0)
    }

    fn max_user_id(&self)->i32{
        match self.users.keys().max(){
            Some(id)=>*id,
            None=>0,
        }
    }

    fn max_vehicle_id(&self)->i32{
        self.vehicles.iter().map(|vehicle|vehicle.id).max().unwrap_or(0)
    }

    fn get_n_availabilities(&self)->usize{
        self.vehicles.iter().flat_map(|vehicle|&vehicle.availability).count()
    }

    fn get_n_tours(&self)->usize{
        self
        .vehicles
        .iter()
        .flat_map(|vehicle| &vehicle.tours)
        .count()
    }

    async fn is_vehicle_idle(
        &self,
        vehicle_id: i32,
        time_frame_start: NaiveDateTime,
        time_frame_end: NaiveDateTime,
    ) -> Result<bool, StatusCode> {
        if self.max_vehicle_id() < vehicle_id  || vehicle_id as usize <= 0{
            return Err(StatusCode::NOT_FOUND);
        }
        let interval = Interval::new(time_frame_start, time_frame_end);
        Ok(!self.vehicles[id_to_vec_pos(vehicle_id)]
            .tours
            .iter()
            .any(|tour| tour.overlaps(&interval)
            ))
    }

    fn get_or_create_address(&mut self, zip_code: &str, city: &str, street: &str, house_nr: &str) -> i32 {
        match self.addresses.iter().find(|address| address.zip_code == zip_code && address.city == city && address.street == street && address.house_nr == house_nr){
            Some(a) => a.id,
            None => {
                let id = self.addresses.len() as i32 + 1;
                let address = AddressData{id, zip_code: zip_code.to_string(), city: city.to_string(), street: street.to_string(), house_nr: house_nr.to_string()};
                self.addresses.push(address);
                id
            }
        }
    }

    //TODO: remove pub when events can be created by handling routing requests
    pub async fn insert_or_addto_tour(
        &mut self,
        tour_id: Option<i32>, // tour_id == None <=> tour already exists
        departure: NaiveDateTime,
        arrival: NaiveDateTime,
        vehicle: i32,
        start_address: &String,
        target_address: &String,
        lat_start: f32,
        lng_start: f32,
        sched_t_start: NaiveDateTime,
        comm_t_start: NaiveDateTime,
        customer: i32,
        passengers: i32,
        wheelchairs: i32,
        luggage: i32,
        request_id: i32,
        connects_public_transport1: bool,
        connects_public_transport2: bool,
        lat_target: f32,
        lng_target: f32,
        sched_t_target: NaiveDateTime,
        comm_t_target: NaiveDateTime,
    ) -> StatusCode {
        if !is_valid(&Interval::new(departure,arrival))
            || !is_valid(&Interval::new(sched_t_start, sched_t_target))
        {
            return StatusCode::NOT_ACCEPTABLE;
        }
        if self.users.len() < customer as usize || self.vehicles.len() < vehicle as usize {
            return StatusCode::EXPECTATION_FAILED;
        }
        let id = match tour_id {
            Some(t_id) => {
                //tour already exists
                if self.get_n_tours() < t_id as usize
                {
                    return StatusCode::EXPECTATION_FAILED;
                }
                t_id
            }
            None => {
                //tour does not exist, create it in database, which yields the id
                let t_id = match Tour::insert(tour::ActiveModel {
                    id: ActiveValue::NotSet,
                    departure: ActiveValue::Set(departure),
                    arrival: ActiveValue::Set(arrival),
                    vehicle: ActiveValue::Set(vehicle),
                })
                .exec(&self.db_connection)
                .await
                {
                    Ok(result) => result.last_insert_id,
                    Err(e) => {
                        error!("Error creating tour: {e:?}");
                        return StatusCode::INTERNAL_SERVER_ERROR;
                    }
                };
                //now create tour in self(data)
                (self.vehicles[id_to_vec_pos(vehicle)]).tours.push(
                    TourData {
                        id: t_id,
                        departure,
                        arrival,
                        vehicle,
                        events: Vec::new(),
                    },
                );
                t_id
            }
        };
        let result = self
            .insert_event_pair_into_db(
                start_address,
                target_address,
                lat_start,
                lng_start,
                sched_t_start,
                comm_t_start,
                customer,
                id,
                passengers,
                wheelchairs,
                luggage,
                request_id,
                connects_public_transport1,
                connects_public_transport2,
                lat_target,
                lng_target,
                sched_t_target,
                comm_t_target,
            )
            .await;
        match result{
            Err(e)=> e,
            Ok((start_event_id, target_event_id))=>{
                let address_id = self.get_or_create_address("zip_code", "city", "street", "house_nr");
                let events = &mut self.vehicles[id_to_vec_pos(vehicle)].tours[id_to_vec_pos(id)].events;
                //pickup-event
                events.push(
                    EventData {
                        coordinates: Point::new(lat_start as f64, lng_start as f64),
                        scheduled_time: sched_t_start,
                        communicated_time: comm_t_start,
                        customer,
                        tour: id,
                        passengers,
                        wheelchairs,
                        luggage,
                        request_id,
                        id: start_event_id,
                        is_pickup: true,
                        address_id,
                    },
                );
                //dropoff-event
                events.push(EventData {
                        coordinates: Point::new(lat_target as f64, lng_target as f64),
                        scheduled_time: sched_t_target,
                        communicated_time: comm_t_target,
                        customer,
                        tour: id,
                        passengers,
                        wheelchairs,
                        luggage,
                        request_id,
                        id: target_event_id,
                        is_pickup: false,
                        address_id,
                    },
                );
                StatusCode::CREATED
            }
        }
    }

    async fn insert_event_pair_into_db(
        &mut self,
        start_address: &String,
        target_address: &String,
        lat_start: f32,
        lng_start: f32,
        sched_t_start: NaiveDateTime,
        comm_t_start: NaiveDateTime,
        customer: i32,
        tour: i32,
        passengers: i32,
        wheelchairs: i32,
        luggage: i32,
        request_id: i32,
        connects_public_transport1: bool,
        connects_public_transport2: bool,
        lat_target: f32,
        lng_target: f32,
        sched_t_target: NaiveDateTime,
        comm_t_target: NaiveDateTime,
    ) -> Result<(i32, i32),StatusCode> {
        let start_id = match Event::insert(event::ActiveModel {
            id: ActiveValue::NotSet,
            longitude: ActiveValue::Set(lng_start),
            latitude: ActiveValue::Set(lat_start),
            scheduled_time: ActiveValue::Set(sched_t_start),
            communicated_time: ActiveValue::Set(comm_t_start),
            request: ActiveValue::Set(request_id),
            is_pickup: ActiveValue::Set(true),
            address: ActiveValue::Set(1),
        })
        .exec(&self.db_connection)
        .await
        {
            Ok(result) => result.last_insert_id,
            Err(e) => {
                error!("Error creating event: {e:?}");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        match Event::insert(event::ActiveModel {
            id: ActiveValue::NotSet,
            longitude: ActiveValue::Set(lng_target),
            latitude: ActiveValue::Set(lat_target),
            scheduled_time: ActiveValue::Set(sched_t_target),
            communicated_time: ActiveValue::Set(comm_t_target),
            request: ActiveValue::Set(request_id),
            is_pickup: ActiveValue::Set(false),
            address: ActiveValue::Set(1),
        })
        .exec(&self.db_connection)
        .await
        {
            Ok(target_result) => Ok((start_id, target_result.last_insert_id)),
            Err(e) => {
                error!("Error creating event: {e:?}");
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    fn get_company(&self, company_id: i32) -> &CompanyData{
        &self.companies[id_to_vec_pos(company_id)]
    }

    async fn get_viable_vehicles(
        &self,
        interval: &Interval,
        passengers: i32,
        start: &Point,
    ) -> Vec<&VehicleData> {
        let zone_viable_companies = self.get_start_point_viable_companies(start).await;
        self.vehicles
            .iter()
            .filter(|vehicle| 
                zone_viable_companies.contains(&self.get_company(vehicle.company))
                    && vehicle.fulfills_requirements(passengers, 0, 0)//TODO: replace zeros when mvp-restrictions are lifted
                    && !vehicle.tours.iter()
                    .any(|tour| {tour.any_tour_event_overlaps(interval)
                    }
                    && vehicle.availability.iter().any(|(_,availability)|availability.interval.contains(interval)))
            )
            .collect_vec()
    }

    async fn get_start_point_viable_companies(
        &self,
        start: &Point,
    ) -> Vec<&CompanyData> {
        let viable_zone_ids = self.get_zones_containing_point_ids(start).await;
        let viable_companies = self
            .companies
            .iter()
            .filter(|company| viable_zone_ids.contains(&(company.zone)))
            .collect_vec();
        viable_companies
    }

    async fn get_zones_containing_point_ids(&self, start: &Point) -> Vec<i32>{
        self
            .zones
            .iter()
            .filter(|zone| zone.area.contains(start))
            .map(|zone| zone.id)
            .collect_vec()
    }

    async fn get_event(&self, vehicle_id: i32, tour_id: i32, event_id: i32)->&EventData{
        &self.vehicles[id_to_vec_pos(vehicle_id)].tours[id_to_vec_pos(tour_id)].events[id_to_vec_pos(event_id)]
    }

    async fn get_tour(
        &self,
        tour_id: i32,
    ) -> Result<&TourData, StatusCode> {
        match self
            .vehicles
            .iter()
            .flat_map(|vehicle| &vehicle.tours)
            .find(|tour| tour.id == tour_id)
        {
            Some(t) => Ok(t),
            None => Err(StatusCode::NOT_FOUND),
        }
    }

    async fn find_event(&self, event_id: i32)->Option<&EventData>{
        self.vehicles.iter().flat_map(|vehicle|vehicle.tours.iter().flat_map(|tour|&tour.events)).find(|event|event.id == event_id)
    }
}

#[cfg(test)]
mod test {
    use super::ZoneData;
    use crate::backend::lib::PrimaData;
    use crate::{
        backend::data::Data,
        constants::{geo_points::TestPoints, gorlitz::GORLITZ},
        dotenv, env,
        init,
        Database, Migrator,
    };
    use sea_orm::DbConn;
    use chrono::{Duration, NaiveDate, NaiveDateTime};
    use geo::{Contains, Point};
    use hyper::StatusCode;
    use migration::MigratorTrait;
    use serial_test::serial;
    use std::collections::HashMap;
    
    async fn insert_or_addto_test_tour(
        data: &mut Data,
        tour_id: Option<i32>,
        departure: NaiveDateTime,
        arrival: NaiveDateTime,
        vehicle: i32,
        
    ) -> StatusCode {
        data.insert_or_addto_tour(
            tour_id,
            departure,
            arrival,
            vehicle,
            &"karolinenplatz 5".to_string(),
            &"Lichtwiesenweg 3".to_string(),
            13.867512,
            51.22069,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 15, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 12, 0)
                .unwrap(),
            2,
            3,
            0,
            0,
            1,
            false,
            false,
            14.025081,
            51.195075,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 55, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 18, 0)
                .unwrap(),
        )
        .await
    }

    async fn check_zones_contain_correct_points(
        d: &Data,
        points: &[Point],
        expected_zones: i32,
    ) {
        for point in points.iter() {
            let companies_containing_point = d.get_start_point_viable_companies(point).await;
            for company in &d.companies {
                if companies_containing_point.contains(&company){
                    assert!(company.zone == expected_zones);
                }else{
                    assert!(company.zone != expected_zones);
                }
            }
        }
    }

    fn check_points_in_zone(
        expect: bool,
        zone: &ZoneData,
        points: &[Point],
    ) {
        for point in points.iter() {
            assert_eq!(zone.area.contains(point), expect);
        }
    }

    async fn check_data_db_synchronized(
        data: &Data,
    ) {
        let mut read_data = Data::new(&data.db_connection);
        read_data.read_data_from_db().await;
        assert!(read_data == *data);
    }

    async fn insert_or_add_test_tour(
        data: &mut Data,
        vehicle_id: i32,
        
    ) -> StatusCode {
        data.insert_or_addto_tour(
            None,
            NaiveDate::from_ymd_opt(5000, 4, 15)
                .unwrap()
                .and_hms_opt(9, 10, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(5000, 4, 15)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            vehicle_id,
            &"karolinenplatz 5".to_string(),
            &"Lichtwiesenweg 3".to_string(),
            13.867512,
            51.22069,
            NaiveDate::from_ymd_opt(5000, 4, 15)
                .unwrap()
                .and_hms_opt(9, 15, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(5000, 4, 15)
                .unwrap()
                .and_hms_opt(9, 12, 0)
                .unwrap(),
            2,
            3,
            0,
            0,
            1,
            false,
            false,
            14.025081,
            51.195075,
            NaiveDate::from_ymd_opt(5000, 4, 15)
                .unwrap()
                .and_hms_opt(9, 55, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(5000, 4, 15)
                .unwrap()
                .and_hms_opt(9, 18, 0)
                .unwrap(),
        )
        .await
    }

    async fn test_main() -> DbConn {
        dotenv().ok();
        let db_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");
        let conn = Database::connect(db_url)
            .await
            .expect("Database connection failed");
        Migrator::up(&conn, None).await.unwrap();
        conn
    }

    #[tokio::test]
    #[serial]
    async fn test_zones() {
        let db_conn = test_main().await;
        let mut d = init::init(&db_conn, true, 5000).await;
        let test_points = TestPoints::new();
        //Validate invalid multipolygon handling when creating zone (expect StatusCode::BAD_REQUEST)
        assert_eq!(
            d.create_zone(
                "some new zone name",
                "invalid multipolygon"
            )
            .await,
            StatusCode::BAD_REQUEST
        );
        //zonen tests:
        //0->Bautzen Ost
        check_points_in_zone(true, &d.zones[0], &test_points.bautzen_ost);
        check_points_in_zone(false, &d.zones[0], &test_points.bautzen_west);
        check_points_in_zone(false, &d.zones[0], &test_points.gorlitz);
        check_points_in_zone(false, &d.zones[0], &test_points.outside);
        //1->Bautzen West
        check_points_in_zone(false, &d.zones[1], &test_points.bautzen_ost);
        check_points_in_zone(true, &d.zones[1], &test_points.bautzen_west);
        check_points_in_zone(false, &d.zones[1], &test_points.gorlitz);
        check_points_in_zone(false, &d.zones[1], &test_points.outside);
        //2->Görlitz
        check_points_in_zone(false, &d.zones[2], &test_points.bautzen_ost);
        check_points_in_zone(false, &d.zones[2], &test_points.bautzen_west);
        check_points_in_zone(true, &d.zones[2], &test_points.gorlitz);
        check_points_in_zone(false, &d.zones[2], &test_points.outside);

        check_zones_contain_correct_points(&d, &test_points.bautzen_ost, 1).await;
        check_zones_contain_correct_points(&d, &test_points.bautzen_west, 2).await;
        check_zones_contain_correct_points(&d, &test_points.gorlitz, 3).await;
        check_zones_contain_correct_points(&d, &test_points.outside, -1).await;

        check_data_db_synchronized(&d).await;
    }
    #[tokio::test]
    #[serial]
    async fn test_key_violations() {
        let db_conn = test_main().await;
        let mut d = init::init(&db_conn, true, 5000).await;
        //validate UniqueKeyViolation handling when creating data (expect StatusCode::CONFLICT)
        //unique keys:  table               keys
        //              user                name, email
        //              zone                name
        //              company             name
        //              vehicle             license-plate
        let n_users = d.users.len();
        //insert user with existing name
        assert_eq!(
            d.create_user(
                "TestDriver1",
                true,
                false,
                Some(1),
                false,
                "test@gmail.com",
                Some("".to_string()),
                "",
                Some("".to_string()),
                Some("".to_string()),
            )
            .await,
            StatusCode::CONFLICT
        );
        assert_eq!(d.users.len(), n_users);
        //insert user with existing email
        assert_eq!(
            d.create_user(
                "TestDriver2",
                true,
                false,
                Some(1),
                false,
                "test@aol.com",
                Some("".to_string()),
                "",
                Some("".to_string()),
                Some("".to_string()),
            )
            .await,
            StatusCode::CONFLICT
        );
        assert_eq!(d.users.len(), n_users);
        //insert user with new name and email
        d.create_user(
            "TestDriver2",
            true,
            true,
            Some(1),
            false,
            "test@gmail.com",
            Some("".to_string()),
            "",
            Some("".to_string()),
            Some("".to_string()),
        )
        .await;
        assert_eq!(d.users.len(), n_users + 1);

        //insert zone with existing name
        let n_zones = d.zones.len();
        assert_eq!(
            d.create_zone("Görlitz", GORLITZ)
                .await,
            StatusCode::CONFLICT
        );
        assert_eq!(d.zones.len(), n_zones);

        //insert company with existing name
        let n_companies = d.companies.len();
        assert_eq!(
            d.create_company(
                "Taxi-Unternehmen Bautzen-1",
                1,
                "mustermann@web.de",
                1.0,
                1.0
            )
            .await,
            StatusCode::CONFLICT
        );

        //insert vehicle with existing license plate
        let n_vehicles = d.vehicles.len();
        assert_eq!(
            d.create_vehicle(&"TUB1-1".to_string(), 1).await,
            StatusCode::CONFLICT
        );
        assert_eq!(d.vehicles.len(), n_vehicles);

        //Validate ForeignKeyViolation handling when creating data (expect StatusCode::EXPECTATION_FAILED)
        //foreign keys: table               keys
        //              company             zone
        //              vehicle             company
        //              availability        vehicle
        //              tour                vehicle
        //              event               user tour
        let base_time = NaiveDate::from_ymd_opt(5000, 1, 1)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let in_2_hours = base_time + Duration::hours(2);
        let in_3_hours = base_time + Duration::hours(3);
        let n_tours = d.get_n_tours();
        let n_events = d.max_event_id();
        assert_eq!(
            insert_or_add_test_tour(&mut d, 100).await,
            StatusCode::EXPECTATION_FAILED
        );
        assert_eq!(
            insert_or_add_test_tour(&mut d, 100).await,
            StatusCode::EXPECTATION_FAILED
        );
        assert_eq!(
            d.insert_event_pair_into_db(
                &"".to_string(),
                &"".to_string(),
                0.0,
                0.0,
                in_2_hours,
                in_2_hours,
                100, //customer
                1,
                3,
                0,
                0,
                1,
                false,
                false,
                0.0,
                0.0,
                in_3_hours,
                in_3_hours
            )
            .await,
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        );
        assert_eq!(
            d.insert_event_pair_into_db(
                &"".to_string(),
                &"".to_string(),
                0.0,
                0.0,
                in_2_hours,
                in_2_hours,
                1,
                100, //tour
                3,
                0,
                0,
                1,
                false,
                false,
                0.0,
                0.0,
                in_3_hours,
                in_3_hours
            )
            .await,
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        );
        assert_eq!(n_events, d.max_event_id());
        assert_eq!(n_tours, d.get_n_tours());
        //insert company with non-existing zone
        assert_eq!(
            d.create_company(
                "some new name",
                1 + n_zones as i32,
                "a@b",
                1.0,
                1.0
            )
            .await,
            StatusCode::EXPECTATION_FAILED
        );
        assert_eq!(d.companies.len(), n_companies);
        //insert company with existing zone
        assert_eq!(
            d.create_company(
                "some new name",
                n_zones as i32,
                "b@c",
                1.0,
                1.0
            )
            .await,
            StatusCode::CREATED
        );
        assert_eq!(d.companies.len(), n_companies + 1);
        let n_companies = d.companies.len();

        //insert vehicle with non-existing company
        assert_eq!(
            d.create_vehicle(
                &"some new license plate".to_string(),
                1 + n_companies as i32
            )
            .await,
            StatusCode::EXPECTATION_FAILED
        );
        assert_eq!(d.vehicles.len(), n_vehicles);
        //insert vehicle with existing company
        assert_eq!(
            d.create_vehicle(
                &"some new license plate".to_string(),
                n_companies as i32
            )
            .await,
            StatusCode::CREATED
        );
        assert_eq!(d.vehicles.len(), n_vehicles + 1);

        check_data_db_synchronized(&d).await;
    }
/*
    #[tokio::test]
    #[serial]
    async fn test_invalid_id_parameter_handling() {
        let db_conn = test_main().await;
        let mut d = init::init(&db_conn, true, 5000).await;

        let base_time = NaiveDate::from_ymd_opt(5000, 1, 1)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let in_2_hours = base_time + Duration::hours(2);
        let in_3_hours = base_time + Duration::hours(3);
        let d_copy = d.clone();

        let max_event_id = d.max_event_id();
        let max_company_id = d.max_company_id();
        let max_tour_id = d.max_tour_id();
        let max_user_id = d.max_user_id();
        //get_assignment_conflicts_for_event
        assert_eq!(
            d.get_assignment_conflicts_for_event(0, None).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_assignment_conflicts_for_event(1, None).await.is_ok());
        assert_eq!(
            d.get_assignment_conflicts_for_event(max_event_id+1, None).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_assignment_conflicts_for_event(max_event_id, None).await.is_ok());

        assert_eq!(
            d.get_assignment_conflicts_for_event(1, Some(0)).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_assignment_conflicts_for_event(1, Some(1)).await.is_ok());
        assert_eq!(
            d.get_assignment_conflicts_for_event(1, Some(1+max_company_id)).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_assignment_conflicts_for_event(1, Some(max_company_id)).await.is_ok());

        //get_company_conflicts_for_tour
        assert_eq!(
            d.get_company_conflicts_for_tour(0, 1, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_company_conflicts_for_tour(1, 1, true).await.is_ok());
        assert_eq!(
            d.get_company_conflicts_for_tour(max_company_id+1, 1, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_company_conflicts_for_tour(max_company_id, 1, true).await.is_ok());

        assert_eq!(
            d.get_company_conflicts_for_tour(1, 0, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_company_conflicts_for_tour(1, 1, true).await.is_ok());
        assert_eq!(
            d.get_company_conflicts_for_tour(1, max_tour_id+1, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_company_conflicts_for_tour(1, max_tour_id, true).await.is_ok());

        //get_events_for_user
        assert_eq!(
            d.get_events_for_user(0, in_2_hours, in_3_hours).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_events_for_user(1, in_2_hours, in_3_hours).await.is_ok());
        assert_eq!(
            d.get_events_for_user(max_user_id+1, in_2_hours, in_3_hours).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_events_for_user(max_user_id, in_2_hours, in_3_hours).await.is_ok());

        //get_vehicles
        assert_eq!(
            d.get_vehicles(0, None).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_vehicles(1, None).await.is_ok());
        assert_eq!(
            d.get_vehicles(max_company_id+1,  None).await,Err(StatusCode::NOT_FOUND)
        );
        assert!(
            d.get_vehicles(max_company_id,  None).await.is_ok());


            //TODO
        assert_eq!(
            d.get_vehicle_conflicts_for_tour(50, 1, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.get_vehicle_conflicts_for_tour(1, 50, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.get_events_for_tour(50).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.is_vehicle_available(50, in_2_hours, in_3_hours,).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.is_vehicle_idle(1, 50, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.is_vehicle_idle(50, 1, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.get_idle_vehicles_for_company(1, 50, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.get_idle_vehicles_for_company(50, 1, true).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.get_events_for_vehicle(50, in_2_hours, in_3_hours).await,Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            d.get_tours_for_vehicle(50, in_2_hours, in_3_hours).await,Err( StatusCode::NOT_FOUND)
        );
        
        assert_eq!(d.get_tour(50).await,Err( StatusCode::NOT_FOUND));
        assert_eq!(
            d.change_vehicle_for_tour(50, 1).await,StatusCode::NOT_FOUND
        );
        assert_eq!(
            d.change_vehicle_for_tour(1, 50).await,StatusCode::NOT_FOUND
        );
        assert_eq!(
            d.remove_availability(in_2_hours, in_3_hours, 50).await,StatusCode::NOT_FOUND,
        );
        assert_eq!(d_copy == d, true);
        check_data_db_synchronized(&d).await;
    } 
*/

    #[tokio::test]
    #[serial]
    async fn test_invalid_interval_parameter_handling() {
        let db_conn = test_main().await;
        let mut d = init::init(&db_conn, true, 5000).await;
        let d_copy = d.clone();

        let base_time = NaiveDate::from_ymd_opt(5000, 1, 1)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        //let in_3_hours = base_time + Duration::hours(3);
        //interval range not limited
        assert!(d.get_tours(1, NaiveDateTime::MIN, NaiveDateTime::MAX).await.is_ok());
        //interval range not limited
        assert!(d.get_events_for_user(1, NaiveDateTime::MIN, NaiveDateTime::MAX).await.is_ok());
        //interval range not limited
        assert!(d.get_events_for_tour(1).await.is_ok());
        //interval range not limited
        assert!(d.get_events_for_vehicle(1, NaiveDateTime::MIN, NaiveDateTime::MAX).await.is_ok());
        let n_availabilities = d.get_n_availabilities();
        //starttime before year 2024
        assert_eq!(
            d.create_availability(
                NaiveDateTime::MIN,
                base_time + Duration::hours(1),
                1
            )
            .await,
            StatusCode::NOT_ACCEPTABLE
        );
        assert_eq!(
            d.get_n_availabilities(),
            n_availabilities
        );
        //endtime after year 100000
        assert_eq!(
            d.create_availability(
                base_time,
                NaiveDate::from_ymd_opt(100000, 4, 15)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                1
            )
            .await,
            StatusCode::NOT_ACCEPTABLE
        );
        assert_eq!(
            d.get_n_availabilities(),
            n_availabilities
        );
        //starttime before year 2024
        assert_eq!(
            d.remove_availability(
                NaiveDate::from_ymd_opt(2023, 4, 15)
                    .unwrap()
                    .and_hms_opt(11, 10, 0)
                    .unwrap(),
                NaiveDate::from_ymd_opt(2024, 4, 15)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                1
            )
            .await,
            StatusCode::NOT_ACCEPTABLE
        );
        //endtime after year 100000
        assert_eq!(
            d.remove_availability(
                NaiveDate::from_ymd_opt(2024, 4, 15)
                    .unwrap()
                    .and_hms_opt(11, 10, 0)
                    .unwrap(),
                NaiveDate::from_ymd_opt(100000, 4, 15)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                1
            )
            .await,
            StatusCode::NOT_ACCEPTABLE
        );
        assert_eq!(
            d.get_n_availabilities(),
            n_availabilities
        );

        assert!(d == d_copy);
        check_data_db_synchronized(&d).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_init() {
        let db_conn = test_main().await;
        let mut d = init::init(&db_conn, true,  5000).await;

        assert_eq!(d.vehicles.len(), 29);
        assert_eq!(d.zones.len(), 3);
        assert_eq!(d.companies.len(), 8);

        d.change_vehicle_for_tour(1, 2).await;

        //get_company_conflicts_for_tour
        for company in d.companies.iter() {
            let conflicts = match d.get_company_conflicts(company.id, 1, true).await {
                Ok(c) => c,
                Err(_) => HashMap::new(),
            };
            assert_eq!(conflicts.is_empty(), company.id != 1);
            for (v, tours) in conflicts.iter() {
                assert_eq!(company.id, 1);
                assert_eq!(tours.is_empty(), *v != 2);
            }
        }

        //get_events_for_user
        for (user_id, _) in d.users.iter() {
            assert_eq!(
                d.get_events_for_user(
                    *user_id,
                    NaiveDate::from_ymd_opt(2024, 1, 1)
                        .unwrap()
                        .and_hms_opt(1, 0, 0)
                        .unwrap(),
                    NaiveDate::from_ymd_opt(6000, 4, 15)
                        .unwrap()
                        .and_hms_opt(14, 0, 0)
                        .unwrap(),
                )
                .await
                .unwrap()
                .is_empty(),
                *user_id != 2 // only user 2 has events
            );
            if *user_id == 2{
                assert_eq!(d.get_events_for_user(
                    *user_id,
                    NaiveDate::from_ymd_opt(2024, 1, 1)
                        .unwrap()
                        .and_hms_opt(1, 0, 0)
                        .unwrap(),
                    NaiveDate::from_ymd_opt(6000, 4, 15)
                        .unwrap()
                        .and_hms_opt(14, 0, 0)
                        .unwrap(),
                    )
                    .await
                    .unwrap().len(),2
                );
            }
        }
/*
        //get_vehicles
        for company in d.companies.iter() {
            let vehicles = d.get_vehicles(company.id, None).await.unwrap();
            for (specs, vehicles) in vehicles.iter() {
                assert_eq!(*specs, 1);
                assert_eq!((company.id == 1 || company.id == 8), vehicles.len() == 5);
                assert_eq!((company.id == 3 || company.id == 7), vehicles.len() == 4);
                assert_eq!(
                    (company.id == 2 || company.id == 5 || company.id == 6),
                    vehicles.len() == 3
                );
                assert_eq!(company.id == 4, vehicles.len() == 2);
            }
        }
 */
        //insert vehicle with non-existing vehicle-specifics test should be added if specifics are no longer restricted for mvp->TODO

        assert_eq!(d.vehicles[1].tours.len(), 1);
        assert_eq!(
            d.get_events_for_vehicle(2, NaiveDateTime::MIN, NaiveDateTime::MAX)
                .await
                .unwrap()
                .len(),
            2
        );
        insert_or_add_test_tour(&mut d, 2).await;
        assert_eq!(d.vehicles[1].tours.len(), 2);
        assert_eq!(
            d.get_events_for_vehicle(2, NaiveDateTime::MIN, NaiveDateTime::MAX)
                .await
                .unwrap()
                .len(),
            4
        );

        for company in d.companies.iter() {
            let conflicts = match d.get_company_conflicts(company.id, 1, true).await {
                Ok(c) => c,
                Err(_) => HashMap::new(),
            };
            assert_eq!(conflicts.is_empty(), company.id != 1);
            for (v, tours) in conflicts.iter() {
                assert_eq!(company.id, 1);
                assert_eq!(tours.is_empty(), *v != 2);
                assert_eq!(tours.len() == 2, *v == 2);
            }
        }

        insert_or_add_test_tour(&mut d, 7).await;
        assert_eq!(d.vehicles[1].tours.len(), 2);
        assert_eq!(
            d.get_events_for_vehicle(2, NaiveDateTime::MIN, NaiveDateTime::MAX)
                .await
                .unwrap()
                .len(),
            4
        );
        assert_eq!(d.vehicles[6].tours.len(), 1);
        assert_eq!(
            d.get_events_for_vehicle(7, NaiveDateTime::MIN, NaiveDateTime::MAX)
                .await
                .unwrap()
                .len(),
            2
        );
        for tour_id in 1..4 {
            //vehicle 2 has tours with ids 1 and 2, vehicle 7 has tour with id 3, no other vehicles have tours
            if tour_id == 3 {
                //consider_provided_tour_conflict parameter only affects vehicle 7, since it is assigned tour_id  (3)
                assert_eq!(
                    d.get_vehicle_conflicts(2, tour_id, true)
                        .await
                        .unwrap()
                        .len(),
                    2
                );
                assert_eq!(
                    d.get_vehicle_conflicts(2, tour_id, false)
                        .await
                        .unwrap()
                        .len(),
                    2
                );
                assert_eq!(
                    d.get_vehicle_conflicts(7, tour_id, true)
                        .await
                        .unwrap()
                        .len(),
                    1
                );
                assert_eq!(
                    d.get_vehicle_conflicts(7, tour_id, false) 
                        .await
                        .unwrap()
                        .len(),
                    0
                );
            } else if tour_id == 1 || tour_id == 2 {
                //consider_provided_tour_conflict parameter only affects vehicle 2, since it is assigned tour_id  (1 or 2)
                assert_eq!(
                    d.get_vehicle_conflicts(2, tour_id, true)
                        .await
                        .unwrap()
                        .len(),
                    2
                );
                assert_eq!(
                    d.get_vehicle_conflicts(2, tour_id, false)
                        .await
                        .unwrap()
                        .len(),
                    1
                );
                assert_eq!(
                    d.get_vehicle_conflicts(7, tour_id, true)
                        .await
                        .unwrap()
                        .len(),
                    1
                );
                assert_eq!(
                    d.get_vehicle_conflicts(7, tour_id, false)
                        .await
                        .unwrap()
                        .len(),
                    1
                );
            } 
            for v_id in 1..d.vehicles.len() + 1 {
                if v_id == 7 || v_id == 2 {
                    continue;
                }
                assert_eq!(
                    d.get_vehicle_conflicts(v_id as i32, tour_id, true)
                        .await
                        .unwrap()
                        .len(),
                    0
                );
            }
        }

        check_data_db_synchronized(&d).await;
    }

    #[tokio::test]
    #[serial]
    async fn availability_test() {
        let db_conn = test_main().await;
        let mut d = init::init(&db_conn, true, 5000).await;
        let n_vehicles = d.vehicles.len();

        let base_time = NaiveDate::from_ymd_opt(5000, 4, 15)
            .unwrap()
            .and_hms_opt(9, 10, 0)
            .unwrap();
        let in_2_hours = base_time + Duration::hours(2);
        let in_3_hours = base_time + Duration::hours(3);

        assert_eq!(d.vehicles[0].availability.len(), 1);
        //remove availabilies created in init
        d.remove_availability(
            NaiveDate::from_ymd_opt(5000, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(5005, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            1,
        )
        .await;
        assert_eq!(d.vehicles[0].availability.len(), 0);
        //add non-touching
        d.create_availability(in_2_hours, in_3_hours, 1)
            .await;
        assert_eq!(d.vehicles[0].availability.len(), 1);
        //add touching
        d.create_availability(
            in_2_hours + Duration::hours(1),
            in_3_hours + Duration::hours(1),
            1,
        )
        .await;
        assert_eq!(d.vehicles[0].availability.len(), 1);
        //add containing/contained (equal)
        d.create_availability(in_2_hours, in_3_hours, 1)
            .await;
        assert_eq!(d.vehicles[0].availability.len(), 1);

        //remove non-touching
        d.remove_availability(
            base_time + Duration::weeks(1),
            base_time + Duration::weeks(2),
            1,
        )
        .await;
        assert_eq!(d.vehicles[0].availability.len(), 1);
        //remove split
        d.remove_availability(
            in_2_hours + Duration::minutes(5),
            in_3_hours - Duration::minutes(5),
            1,
        )
        .await;
        assert_eq!(d.vehicles[0].availability.len(), 2);
        //remove overlapping
        d.remove_availability(
            in_2_hours - Duration::minutes(90),
            in_3_hours - Duration::minutes(100),
            1,
        )
        .await;
        assert_eq!(d.vehicles[0].availability.len(), 2);
        //remove containing
        d.remove_availability(
            in_2_hours - Duration::minutes(1),
            in_3_hours + Duration::hours(3),
            1,
        )
        .await;
        assert_eq!(d.vehicles[0].availability.len(), 0);

        //Validate StatusCode cases
        //insert availability with non-existing vehicle
        let n_availabilities = d.get_n_availabilities();
        assert_eq!(
            d.create_availability(
                base_time,
                base_time + Duration::hours(1),
                1 + n_vehicles as i32
            )
            .await,
            StatusCode::EXPECTATION_FAILED
        );
        assert_eq!(
            d.get_n_availabilities(),
            n_availabilities
        );
        //insert availability with existing vehicle
        let n_availabilities = d.get_n_availabilities();
        assert_eq!(
            d.create_availability(
                base_time,
                base_time + Duration::hours(1),
                n_vehicles as i32
            )
            .await,
            StatusCode::CREATED
        );
        assert_eq!(
            d.get_n_availabilities(),
            n_availabilities + 1
        );
        let n_availabilities = d.get_n_availabilities();

        assert_eq!(
            d.get_n_availabilities(),
            n_availabilities
        );

        //Validate nothing happened case handling when removing availabilies (expect StatusCode::NO_CONTENT)
        //endtime after year 100000
        assert_eq!(
            d.remove_availability(
                NaiveDate::from_ymd_opt(2025, 4, 15)
                    .unwrap()
                    .and_hms_opt(11, 10, 0)
                    .unwrap(),
                NaiveDate::from_ymd_opt(2026, 4, 15)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                1
            )
            .await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            d.get_n_availabilities(),
            n_availabilities
        );

        d.insert_or_addto_tour(
            None,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 10, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            1,
            &"karolinenplatz 5".to_string(),
            &"Lichtwiesenweg 3".to_string(),
            13.867512,
            51.22069,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 15, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 12, 0)
                .unwrap(),
            2,
            3,
            0,
            0,
            1,
            false,
            false,
            14.025081,
            51.195075,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 55, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 18, 0)
                .unwrap(),
        )
        .await;

        d.insert_or_addto_tour(
            None,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 10, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            1,
            &"karolinenplatz 5".to_string(),
            &"Lichtwiesenweg 3".to_string(),
            13.867512,
            51.22069,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 15, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 12, 0)
                .unwrap(),
            2,
            3,
            0,
            0,
            1,
            false,
            false,
            14.025081,
            51.195075,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 55, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 18, 0)
                .unwrap(),
        )
        .await;

        d.insert_or_addto_tour(
            None,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 10, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            1,
            &"karolinenplatz 5".to_string(),
            &"Lichtwiesenweg 3".to_string(),
            13.867512,
            51.22069,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 15, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 12, 0)
                .unwrap(),
            2,
            3,
            0,
            0,
            1,
            false,
            false,
            14.025081,
            51.195075,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 55, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 18, 0)
                .unwrap(),
        )
        .await;

        d.insert_or_addto_tour(
            None,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 10, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            1,
            &"karolinenplatz 5".to_string(),
            &"Lichtwiesenweg 3".to_string(),
            13.867512,
            51.22069,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 15, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 12, 0)
                .unwrap(),
            2,
            3,
            0,
            0,
            1,
            false,
            false,
            14.025081,
            51.195075,
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 55, 0)
                .unwrap(),
            NaiveDate::from_ymd_opt(2024, 4, 15)
                .unwrap()
                .and_hms_opt(9, 18, 0)
                .unwrap(),
        )
        .await;

        check_data_db_synchronized(&d).await;
    }
}
