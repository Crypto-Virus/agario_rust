

use std::collections::{HashMap};
use std::marker::Copy;
use std::net::SocketAddr;
use std::time::SystemTime;
use std::u8;
use std::{
    sync::Arc,
};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::*;
use ethers::prelude::H160;
use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use jsonrpc_core::Value;
use rand::{thread_rng, Rng};
use tokio::time::{self, Duration};
use serde::{Serialize, Deserialize};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;


use crate::PeerMap;
use crate::utils::SplitOneMut;
use crate::grid::Grid;


type Players = HashMap<String, Player>;
type Food = Vec<FoodCell>;

const TICKS_PER_SEC: u64 = 60;
const UPDATES_PER_SEC: u64 = 60;
const DEFAULT_MASS: f64 = 10.;
const DEFAULT_FOOD_MASS: f64 = 1.;
const INIT_CELL_SPEED: f64 = 5.;
const GAME_WIDTH: u32 = 5000;
const GAME_HEIGHT: u32 = 5000;
const LOG_BASE: f64 = 10.;
const INIT_MASS_LOG: f64 = 1.;
const NEW_PLAYER_FOOD: i32 = 10000;
const FOOD_LOOP_AMOUNT: i32 = 10000;
const MERGE_TIME: u128 = 5000;
const MAX_SPLIT_NUM: usize = 16;
const SPLIT_MOMENTUM: f64 = 25.;
const MINIMUM_VISIBLE_RANGE: f64 = 550.;
const WIN_TIME: u64 = 10;
const WIN_THRESHOLD: i32 = 100;
const WIN_PERCENTAGE: f64 = 0.9;
const MAX_PLAYERS: i32 = 100;
const ENTRY_FEE: i32 = 100;


abigen!(
    SimpleContract,
    "./data/abi/GamePool.json",
    event_derives(serde::Deserialize, serde::Serialize)
);


pub enum GameError {
    PlayerAlreadyInGame,
    NoTicketsAvailable,
}

impl GameError {
    pub fn description(&self) -> String {
        let desc = match *self {
            GameError::PlayerAlreadyInGame => "You are already in game!",
            GameError::NoTicketsAvailable => "You have no tickets to play!"
        };
        desc.to_string()
    }
}


pub trait ToBytes {
    fn to_bytes(&self) -> Vec<u8>;
}

trait RadiusTrait {
    fn radius(&self) -> f64;
}

trait MassTrait {
    fn mass(&self) -> f64;
}

pub trait PositionTrait {
    fn position(&self) -> Position;

    fn distance_to(&self, p2: &impl PositionTrait) -> f64 {
        ((self.position().x - p2.position().x).powi(2) + (self.position().y - p2.position().y).powi(2)).sqrt()
    }
}

trait CellTrait: PositionTrait + RadiusTrait {
    fn is_collide(&self, other: &impl PositionTrait) -> bool {
        let self_pos = self.position();
        let self_radius = self.radius();
        let other_pos = other.position();
        let dx = (other_pos.x - self_pos.x).abs();
        let dy = (other_pos.y - self_pos.y).abs();
        if dx > self_radius {
            return false;
        } else if dy > self_radius {
            return false
        } else if dx + dy <= self_radius {
            return true
        } else if dx.powf(2.) + dy.powf(2.) <= self_radius.powf(2.) {
            return true
        } else {
            false
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
struct PlayerCell {
    #[serde(skip)]
    player_id: String,
    pos: Position,
    #[serde(skip)]
    mass: f64,
    radius: f64,
    hue: f64,
    #[serde(skip)]
    momentum: f64,
    #[serde(skip)]
    last_split: Option<SystemTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FoodCell {
    pos: Position,
    hue: f64,
    #[serde(skip)]
    mass: f64,
    #[serde(skip)]
    radius: f64,
}

#[derive(Debug, Clone)]
struct Player {
    id: String,
    addr: SocketAddr,
    tx: Sender<Message>,
    cells: Vec<PlayerCell>,
    target: Option<Position>,
    visible_range: f64,
}


impl PlayerCell {
    fn new(player_id: String, pos: Position) -> PlayerCell {
        PlayerCell {
            player_id: player_id,
            pos: pos,
            mass: DEFAULT_MASS,
            radius: mass_to_radius(DEFAULT_MASS),
            hue: generate_random_hue(),
            momentum: 1.,
            last_split: None,
        }
    }

    fn speed(&self, target_dist: f64) -> f64 {
        // game point per tick
        let x = ((target_dist - 20.) / 20.).min(1.).max(0.);
        (INIT_CELL_SPEED / (self.mass.log(LOG_BASE) - INIT_MASS_LOG + 1.) + self.momentum) * x
    }

    fn split(&mut self) -> Option<PlayerCell> {
        if self.mass < DEFAULT_MASS * 2. {
            return None;
        }
        self.update_mass(self.mass / 2.);
        self.last_split = Some(SystemTime::now());
        Some(PlayerCell {
            player_id: self.player_id.clone(),
            pos: self.pos,
            mass: self.mass,
            radius: self.radius,
            hue: self.hue,
            momentum: SPLIT_MOMENTUM,
            last_split: self.last_split,
        })
    }

    fn can_merge(&self) -> bool{
        if let Some(last_split) = self.last_split {
            if let Ok(elapsed) = last_split.elapsed() {
                if elapsed.as_millis() > MERGE_TIME {
                    return true;
                } else {
                    return false;
                }
            }
        }
        true
    }

    fn update_mass(&mut self, value: f64) {
        self.mass = value;
        self.radius = mass_to_radius(self.mass);
    }
}

impl ToBytes for PlayerCell {
    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend((self.pos.x as f32).to_le_bytes());
        data.extend((self.pos.y as f32).to_le_bytes());
        data.extend((self.radius as f32).to_le_bytes());
        data.extend((self.hue as u8).to_le_bytes());
        data
    }
}

impl CellTrait for PlayerCell {}

impl FoodCell {
    fn new() -> FoodCell {
        FoodCell {
            pos: random_position(),
            hue: generate_random_hue(),
            mass: DEFAULT_FOOD_MASS,
            radius: mass_to_radius(DEFAULT_FOOD_MASS),
        }
    }
}

impl ToBytes for FoodCell {
    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend((self.pos.x as f32).to_le_bytes());
        data.extend((self.pos.y as f32).to_le_bytes());
        data.extend((self.hue as u8).to_le_bytes());
        data
    }
}

impl CellTrait for FoodCell {}

impl Player {
    fn new_player(addr: SocketAddr, eth_address: String, tx: Sender<Message>, pos: Position) -> Player{
        Player {
            id: eth_address.clone(),
            addr: addr,
            tx: tx,
            cells: vec![PlayerCell::new(eth_address, pos)],
            target: None,
            visible_range: MINIMUM_VISIBLE_RANGE,

        }
    }

    fn update_visible_range(&mut self) {
        self.visible_range = 120. * (self.radius() - 22.).max(0.).sqrt() + MINIMUM_VISIBLE_RANGE;
    }

    fn is_visible(&self, other: &impl PositionTrait) -> bool {
        let half_visible = self.visible_range / 2.;
        let self_pos = self.position();
        let other_pos = other.position();
        let min_x = self_pos.x - half_visible;
        let max_x = self_pos.x + half_visible;
        let min_y = self_pos.y - half_visible;
        let max_y = self_pos.y + half_visible;
        if  (min_x <= other_pos.x) &&
            (other_pos.x <= max_x) &&
            (min_y <= other_pos.y) &&
            (other_pos.y <= max_y) {
            true
        } else {
            false
        }
    }
}

impl RadiusTrait for PlayerCell {
    fn radius(&self) -> f64 {
        self.radius
    }
}

impl RadiusTrait for FoodCell {
    fn radius(&self) -> f64 {
        self.radius
    }
}

impl RadiusTrait for Player {
    fn radius(&self) -> f64 {
        mass_to_radius(self.mass())
    }
}

impl MassTrait for PlayerCell {
    fn mass(&self) -> f64 {
        self.mass
    }
}

impl MassTrait for FoodCell {
    fn mass(&self) -> f64 {
        self.mass
    }
}

impl MassTrait for Player {
    fn mass(&self) -> f64 {
        let mut total_mass = 0.;
        for cell in &self.cells {
            total_mass += cell.mass;
        }
        total_mass
    }
}

impl PositionTrait for Position {
    fn position(&self) -> Position {
        *self
    }
}

impl PositionTrait for PlayerCell {
    fn position(&self) -> Position {
        self.pos
    }
}

impl PositionTrait for FoodCell {
    fn position(&self) -> Position {
        self.pos
    }
}

impl PositionTrait for Player {
    fn position(&self) -> Position {
        let x: f64 = self.cells.iter().map(|c| c.pos.x).sum::<f64>() / self.cells.len() as f64;
        let y: f64 = self.cells.iter().map(|c| c.pos.y).sum::<f64>() / self.cells.len() as f64;
        Position {
            x: x,
            y: y,
        }
    }
}


fn mass_to_radius(mass: f64) -> f64 {
    4.0 + mass.sqrt() * 6.0
}

fn random_position() -> Position {
    let mut rng = thread_rng();
    Position {
        x: rng.gen_range(mass_to_radius(DEFAULT_MASS)..=GAME_WIDTH as f64).floor(),
        y: rng.gen_range(mass_to_radius(DEFAULT_MASS)..=GAME_HEIGHT as f64).floor(),
    }
}

fn distance_between_circles<T, U>(a: &T, b: &U) -> f64
    where T: PositionTrait + RadiusTrait,
          U: PositionTrait + RadiusTrait
{
    a.distance_to(b) - a.radius() - b.radius()
}

fn generate_random_hue() -> f64 {
    let mut rng = thread_rng();
    rng.gen_range(0.0..360.0)
}


fn get_new_player_position(players: &Players) -> Position {

    if players.is_empty() {
        return random_position();
    }

    let mut best_pos = None;
    let mut best_dist = 0.;
    for _ in 1..10 {
        let mut min_dist = f64::INFINITY;
        let rand_pos = random_position();
        for player in players.values() {
            let tmp_cell = PlayerCell::new(String::new(), rand_pos);
            let dist = distance_between_circles(player, &tmp_cell);
            if dist < min_dist {
                min_dist = dist
            }
        }

        if min_dist > best_dist {
            best_pos = Some(rand_pos);
            best_dist = best_dist;
        }

    }

    match best_pos {
        Some(pos) => pos,
        None => random_position()
    }
}


struct State {
    players: Players,
    food: Food,
}

#[derive(Debug)]
pub struct Game {
    players: Players,
    food: Food,
    food_stack: i32,
    peer_map: PeerMap,
    eth_addr_peer_map: crate::EthAddrPeerMap,
    socket_addr_to_eth_address: HashMap<SocketAddr, String>,
    address_tickets_map: HashMap<String, i32>,
}


impl Game {
    pub fn new(
        peer_map: crate::PeerMap,
        eth_addr_peer_map: crate::EthAddrPeerMap,
    ) -> Game {

        Game {
            players: HashMap::new(),
            food: Vec::new(),
            food_stack: 0,
            peer_map: peer_map,
            eth_addr_peer_map: eth_addr_peer_map,
            socket_addr_to_eth_address: HashMap::new(),
            address_tickets_map: HashMap::new(),
        }
    }

    pub fn enter_game(&mut self, addr: SocketAddr, eth_address: String) -> Result<(), GameError> {
        if self.players.contains_key(&eth_address) {
            return Err(GameError::PlayerAlreadyInGame);
        }
        let remaining_tickets = self.use_ticket(&eth_address)?;
        self.add_player(addr, eth_address.clone())?;
        self.notify_player_by_id(&eth_address, "notify_tickets_update", json!([remaining_tickets]));
        Ok(())
    }

    pub fn add_player(&mut self, addr: SocketAddr, player_addr: String) -> Result<(), GameError> {
        // fix detection if player is in game
        if self.socket_addr_to_eth_address.contains_key(&addr) {
            return Err(GameError::PlayerAlreadyInGame)
        }

        let tx = self.peer_map.lock().unwrap().get(&addr).unwrap().clone();

        let player = Player::new_player(
            addr,
            player_addr,
            tx,
            get_new_player_position(&self.players)
        );
        println!("new player entered the game. Player ID [{}]", player.id);
        self.socket_addr_to_eth_address.insert(addr, player.id.clone());
        self.players.insert(player.id.clone(), player);
        self.food_stack += NEW_PLAYER_FOOD;

        Ok(())
    }

    pub fn ticket_bought(&mut self, eth_address: String) {
        *self.address_tickets_map.entry(eth_address.clone()).or_default() += 1;
        let tickets = *self.address_tickets_map.get(&eth_address).unwrap();
        self.notify_player_by_id(&eth_address, "notify_tickets_update", json!([tickets]));
    }

    fn use_ticket(&mut self, user: &str) -> Result<i32, GameError> {
        let tickets = self.address_tickets_map.get_mut(user);
        match tickets {
            Some(tickets) => {
                if *tickets == 0 {
                    return Err(GameError::NoTicketsAvailable);
                }
                *tickets -= 1;
                Ok(*tickets)
            }
            None => Err(GameError::NoTicketsAvailable)
        }
    }

    pub fn get_available_tickets(&self, eth_address: &str) -> i32 {
        if let Some(tickets) = self.address_tickets_map.get(eth_address) {
            return *tickets;
        } else {
            return 0;
        }
    }

    pub fn get_server_info(&self) -> serde_json::Value{
        json!({
            "max_players": MAX_PLAYERS,
            "entry_fee": ENTRY_FEE,
        })
    }

    fn add_food(&mut self, mut amount: i32) {
        if amount > self.food_stack {
            amount = self.food_stack;
        }
        self.food_stack -= amount;
        for i in 0..amount {
            self.food.push(FoodCell::new())
        }
    }

    fn remove_player(&mut self, player_id: &String) -> Option<Player> {
        println!("Removing player. Player ID [{}]", player_id);
        self.players.remove(player_id)
    }

    pub fn set_target(&mut self, addr: SocketAddr, x: f64, y: f64) {
        if let Some(player_id) = self.socket_addr_to_eth_address.get(&addr) {
            if let Some(player) = self.players.get_mut(player_id) {
                player.target = Some(Position{x: x, y: y});
            }
        } else {
            println!(
                "Cannot set target because no player is associated with this connection. Addr[{}]",
                addr
            );
        }
    }

    pub fn split(&mut self, addr: SocketAddr) {
        if let Some(player_id) = self.socket_addr_to_eth_address.get(&addr) {
            let player = self.players.get_mut(player_id).unwrap();
            for i in 0..player.cells.len() {
                if player.cells.len() < MAX_SPLIT_NUM {
                    let cell = &mut player.cells[i];
                    if let Some(new_cell) = cell.split() {
                        player.cells.push(new_cell);
                    }
                }

            }
        }
    }

    pub fn player_lost_connection(&mut self, addr: SocketAddr) {
        if let Some(player_id) = self.socket_addr_to_eth_address.remove(&addr) {
            self.remove_player(&player_id);
        }
    }

    fn notify_player(&self, player: &Player , method: &str, params: Value) {
        let message = json!({
                "method": method,
                "params": params,
        }).to_string();
        let tx = player.tx.clone();
        tokio::spawn(async move {
            tx.send(Message::text(message)).await;
        });
    }

    fn notify_player_by_id(&self, id: &str , method: &str, params: Value) {
        if let Some(tx) = self.eth_addr_peer_map.lock().unwrap().get(id) {
            let tx = tx.clone();
            let message = json!({
                "method": method,
                "params": params,
            }).to_string();
            tokio::spawn(async move {
                tx.send(Message::text(message)).await;
            });
        }
    }

    fn move_players(&mut self) {
        for player in self.players.values_mut() {
            let player_pos = player.position();
            for cell in &mut player.cells {
                match &player.target {
                    Some(target) => {
                        let cell_target = Position {
                            x: player_pos.x + target.x - cell.pos.x,
                            y: player_pos.y + target.y - cell.pos.y
                        };
                        let target_dist = (cell_target.x.powf(2.) + cell_target.y.powf(2.)).sqrt();
                        let rad = cell_target.y.atan2(cell_target.x);
                        let cell_speed = cell.speed(target_dist);
                        let delta_y = cell_speed * rad.sin();
                        let delta_x = cell_speed * rad.cos();
                        cell.pos.y += delta_y;
                        cell.pos.x += delta_x;

                        if cell.momentum > 1. {
                            cell.momentum -= 0.7
                        }
                        if cell.momentum < 1. {
                            cell.momentum = 1.
                        }

                        // Apply padding between cell and game border
                        let border_padding = cell.radius / 3.;
                        if cell.pos.x > GAME_WIDTH as f64 - border_padding {
                            cell.pos.x = GAME_WIDTH as f64 - border_padding;
                        }
                        if cell.pos.y > GAME_HEIGHT as f64 - border_padding {
                            cell.pos.y = GAME_HEIGHT as f64 - border_padding;
                        }
                        if cell.pos.x < border_padding {
                            cell.pos.x = border_padding;
                        }
                        if cell.pos.y < border_padding {
                            cell.pos.y = border_padding;
                        }
                    }
                    None => continue
                }
            }

            // check for cell merge
            let mut i = 0;
            while i < player.cells.len() {
                let mut remove = false;
                let (cell, other_cells) = player.cells.split_one_mut(i);
                for other_cell in other_cells {
                    if cell.mass < other_cell.mass {continue;}

                    let dist = cell.distance_to(other_cell);
                    let total_radius = cell.radius + other_cell.radius;
                    if dist < total_radius {
                        if cell.can_merge() && other_cell.can_merge() {
                            if dist < total_radius / 1.75 {
                                other_cell.update_mass(other_cell.mass + cell.mass);
                                other_cell.last_split = None;
                                remove = true;
                                break;
                            }
                        } else {
                            if cell.pos.x < other_cell.pos.x {
                                cell.pos.x -= 1.;
                            } else if cell.pos.x >= other_cell.pos.x {
                                cell.pos.x += 1.;
                            }
                            if cell.pos.y < other_cell.pos.y {
                                cell.pos.y -= 1.;
                            } else if cell.pos.y >= other_cell.pos.y {
                                cell.pos.y += 1.;
                            }
                        }
                    }
                }

                if remove {
                    player.cells.remove(i);
                } else {
                    i += 1
                }
            }

        }
    }

    fn check_players_collisions(&mut self) {
        let mut players_vec: Vec<&mut Player> = self.players.values_mut().collect();

        for i in 0..players_vec.len() {
            let (player, mut other_players) = players_vec.split_one_mut(i);

            for other_player in &mut other_players {
                for cell in &mut player.cells {
                    other_player.cells.retain(|other_cell| {
                        if cell.is_collide(other_cell) {
                            if cell.mass > other_cell.mass * 1.1 {
                                cell.update_mass(cell.mass + other_cell.mass);
                                return false;
                            }
                        }
                        true
                    });
                }
            }

            player.update_visible_range();
        }

        // remove players that have no more cells
        let mut players = std::mem::take(&mut self.players);
        players.retain(|_, player| {
            if player.cells.is_empty() {
                self.socket_addr_to_eth_address.remove(&player.addr);
                self.notify_game_over(player);
                return false
            }
            true

        });
        self.players = players;

    }

    fn check_food_collisions(&mut self) {
        for player in self.players.values_mut() {
            for cell in &mut player.cells {
                self.food.retain(|f| {
                    if cell.is_collide(f) {
                        cell.update_mass(cell.mass + f.mass);
                        return false;
                    }
                    true
                });
            }
        }
    }

    fn check_collisions(&mut self) {
        let cells = self.players.values().flat_map(|p| &p.cells).into_iter();
        let player_cells_grid = Grid::new(GAME_WIDTH, 500, cells.into_iter());
        let food_cells_grid = Grid::new(GAME_WIDTH, 500, self.food.iter());

        let mut consumed_player_cells = Vec::new();
        let mut consumed_food = Vec::new();
        // bug: if player consume a cell and another
        // consume him at the same tick. the player alive
        // won't be consuming the cell
        let mut total_mass_gained = Vec::new();

        for player_id in self.players.keys() {
            let player = self.players.get(player_id).unwrap();
            for cell_idx in 0..player.cells.len() {
                let cell = &player.cells[cell_idx];
                let mut mass_gained = 0.;

                let food_cells = food_cells_grid.query(cell.pos, cell.radius as u32 * 2);
                food_cells.for_each(|f| {
                    if !consumed_food.iter().any(|&f2| std::ptr::eq(f, f2)) {
                        if cell.is_collide(f) {
                            mass_gained += f.mass;
                            consumed_food.push(f)
                        }
                    }
                });


                let player_cells = player_cells_grid.query(cell.pos, cell.radius as u32 * 2);
                player_cells.for_each(|other_cell| {
                    if cell.player_id == other_cell.player_id {return}
                    if !consumed_player_cells.iter().any(|&c| std::ptr::eq(other_cell, c)) {
                        if cell.mass > other_cell.mass * 1.1 {
                            if cell.is_collide(other_cell) {
                                mass_gained += other_cell.mass;
                                consumed_player_cells.push(other_cell)
                            }
                        }
                    }
                });

                total_mass_gained.push((player_id.clone(), cell_idx, mass_gained));

            }
        }

        // update player cell masses and visible range
        for (player_id, cell_idx, mass_gained) in total_mass_gained {
            let player = self.players.get_mut(&player_id).unwrap();
            let cell = &mut player.cells[cell_idx];
            cell.update_mass(cell.mass + mass_gained);

            // update player visible range
            player.update_visible_range();
        }


        // remove consumed food
        self.food.retain(|f| {
            !consumed_food.iter().any(|&f2| std::ptr::eq(f, f2))
        });

        // remove consumed cells
        for player in self.players.values_mut() {
            player.cells.retain(|cell| {
                !consumed_player_cells.iter().any(|&c| std::ptr::eq(cell, c))
            });
        }

        // remove players that have no more cells
        let mut players = std::mem::take(&mut self.players);
        players.retain(|_, player| {
            if player.cells.is_empty() {
                self.socket_addr_to_eth_address.remove(&player.addr);
                self.notify_game_over(player);
                return false
            }
            true

        });
        self.players = players;

    }

    fn notify_game_over(&self, player: &mut Player) {
        self.notify_player(player, "notify_game_over", Value::Null);
    }

    fn get_state(&self) -> State {
        State {
            players: self.players.clone(),
            food: self.food.clone(),
        }
    }

    fn get_scores(&self) -> Vec<(String, u32)> {
        let mut scores: Vec<(String, u32)> = self.players.values()
            .map(|player| (player.id.clone(), player.mass()as u32))
            .collect();
        scores.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        scores
    }

    fn get_winner(&self) -> Option<String> {
        if let Some(player) = self.players.values().max_by(|&a, &b| a.mass().partial_cmp(&b.mass()).unwrap()) {
            if player.mass() > WIN_THRESHOLD as f64 {
                return Some(player.id.clone());
            }
        }
        None
    }
}

async fn tick_loop(game: crate::Game) {
    let mut sleep_time = 0.;
    loop {
        time::sleep(Duration::from_millis(sleep_time as u64)).await;

        let now = SystemTime::now();
        let mut game = game.lock().unwrap();
        // game.move_players();
        // game.check_collisions();

        let elapsed = now.elapsed()
            .unwrap_or_default().as_millis() as f64;
        // println!("game tick {}", elapsed);
        sleep_time = (1000. / TICKS_PER_SEC as f64 - elapsed).max(1.);
    }
}


async fn move_loop(game: crate::Game) {
    loop {
        time::sleep(Duration::from_millis(1000 / 60)).await;
        game.lock().unwrap().move_players();
    }
}

async fn player_collision_loop(game: crate::Game) {
    loop {
        time::sleep(Duration::from_millis(1000 / 40)).await;
        game.lock().unwrap().check_players_collisions();
    }
}

async fn food_collision_loop(game: crate::Game) {
    loop {
        time::sleep(Duration::from_millis(1000 / 20)).await;
        game.lock().unwrap().check_food_collisions();
    }
}

async fn add_food_loop(game: crate::Game) {
    loop {
        time::sleep(Duration::from_secs(1)).await;
        let mut game = game.lock().unwrap();
        game.add_food(FOOD_LOOP_AMOUNT);
    }
}

async fn update_loop(game: crate::Game) {
    let mut sleep_time = 0.;
    loop {
        time::sleep(Duration::from_millis(sleep_time as u64)).await;
        let now = SystemTime::now();
        let players = game.lock().unwrap().players.clone();
        let cells = players.values().flat_map(|p| &p.cells).into_iter();
        let mut player_cells_grid = Grid::new(GAME_WIDTH, 250, cells);
        for player in players.values() {
            let mut message = vec![0u8];
            let Position { x, y } = player.position();
            message.extend((x as f32).to_le_bytes());
            message.extend((y as f32).to_le_bytes());
            message.extend((player.visible_range as f32).to_le_bytes());
            message.extend(player_cells_grid.query_serialized(player.position(), player.visible_range as u32));
            player.tx.send(Message::binary(message)).await;
        }

        let elapsed = now.elapsed()
            .unwrap_or_default().as_millis() as f64;
        // println!("update {}", elapsed);
        sleep_time = (1000. / UPDATES_PER_SEC as f64 - elapsed).max(1.);
    }
}

async fn food_update_loop(game: crate::Game) {
    loop {
        time::sleep(Duration::from_millis(50)).await;
        let now = SystemTime::now();
        let state = game.lock().unwrap().get_state();
        let mut food_cells_grid = Grid::new(GAME_WIDTH, 250, state.food.iter());
        for player in state.players.values() {
            let mut message = vec![1u8];
            message.extend(food_cells_grid.query_serialized(player.position(), player.visible_range as u32));
            player.tx.send(Message::binary(message)).await;
        }

        let elapsed = now.elapsed()
            .unwrap_or_default().as_millis() as f64;
        // println!("food update {}", elapsed);
    }
}

async fn metadata_update_loop(game: crate::Game) {
    loop {
        time::sleep(Duration::from_secs(1)).await;
        let scores;
        let players;
        {
            let game = game.lock().unwrap();
            scores = game.get_scores();
            players = game.players.clone();
        }
        let end_idx = if scores.len() >= 10 {10} else {scores.len()};
        let scores = &scores[0..end_idx];
        let message = json!({
            "method": "notify_update_metadata",
            "params": {
                "scores": scores,
            }
        }).to_string();
        for player in players.values() {
            player.tx.send(Message::text(message.clone())).await;
        }
    }
}

async fn win_loop(game: crate::Game, game_pool_addr: String, client: Arc<SignerMiddleware<Provider<Ws>, Wallet<SigningKey>>>) {
    let contract_addr = H160::from_str(&game_pool_addr).expect("Game pool address is invalid");
    let contract = SimpleContract::new(contract_addr, client);
    loop {
        time::sleep(Duration::from_secs(WIN_TIME)).await;
        let mut player: Option<Player> = None;
        {
            let mut game = game.lock().unwrap();
            if let Some(player_id) = game.get_winner() {
                player = game.remove_player(&player_id);
            }
        }
        if let Some(player) = player {

            let amount = (player.mass() / WIN_PERCENTAGE * 1e9) as i128;
            println!("Awarding player with {}", amount);
            contract.award_winner(
                H160::from_str(&player.id).unwrap(),
                U256::from(amount),
            ).legacy().send().await.unwrap();
        }

    }
}


pub fn start_tasks(game: crate::Game, game_pool_addr: String, client : Arc<SignerMiddleware<Provider<Ws>, Wallet<SigningKey>>>) {
    // tokio::spawn(tick_loop(game.clone()));
    tokio::spawn(move_loop(game.clone()));
    tokio::spawn(player_collision_loop(game.clone()));
    tokio::spawn(food_collision_loop(game.clone()));
    tokio::spawn(add_food_loop(game.clone()));
    tokio::spawn(update_loop(game.clone()));
    tokio::spawn(food_update_loop(game.clone()));
    tokio::spawn(metadata_update_loop(game.clone()));
    tokio::spawn(win_loop(game.clone(), game_pool_addr, client.clone()));
}