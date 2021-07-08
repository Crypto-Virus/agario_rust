

use std::collections::HashMap;
use std::marker::Copy;
use std::net::SocketAddr;
use std::time::SystemTime;
use futures_channel::mpsc::UnboundedSender;
use jsonrpc_core::Params;
use rand::{thread_rng, Rng};
use tokio::time::{self, Duration};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde_json::json;
use tungstenite::Message;


use crate::PeerMap;
use crate::utils::SplitOneMut;

type Players = HashMap<String, Player>;
type Food = Vec<FoodCell>;

const TICKS_PER_SEC: u64 = 60;
const UPDATES_PER_SEC: u64 = 60;
const DEFAULT_MASS: f64 = 10.;
const DEFAULT_FOOD_MASS: f64 = 1.;
const INIT_CELL_SPEED: f64 = 5.;
const GAME_WIDTH: f64 = 5000.0;
const GAME_HEIGHT: f64 = 5000.0;
const LOG_BASE: f64 = 10.;
const INIT_MASS_LOG: f64 = 1.;
const NEW_PLAYER_FOOD: i32 = 10000;
const FOOD_LOOP_AMOUNT: i32 = 10000;
const MERGE_TIME: u128 = 5000;
const MAX_SPLIT_NUM: usize = 16;
const SPLIT_MOMENTUM: f64 = 25.;
const MINIMUM_VISIBLE_RANGE: f64 = 550.;


pub enum GameError {
    PlayerAlreadyInGame,
}

impl GameError {
    pub fn description(&self) -> String {
        let desc = match *self {
            GameError::PlayerAlreadyInGame => "You are already in game!",
        };
        desc.to_string()
    }
}


trait RadiusTrait {
    fn radius(&self) -> f64;
}

trait MassTrait {
    fn mass(&self) -> f64;
}

trait PositionTrait {
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
struct Position {
    x: f64,
    y: f64,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
struct PlayerCell {
    pos: Position,
    radius: f64,
    hue: f64,
    #[serde(skip)]
    mass: f64,
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
    tx: UnboundedSender<Message>,
    cells: Vec<PlayerCell>,
    target: Option<Position>,
    visible_range: f64,
}


impl PlayerCell {
    fn new_player_cell(pos: Position) -> PlayerCell {
        PlayerCell {
            pos: pos,
            mass: DEFAULT_MASS,
            radius: mass_to_radius(DEFAULT_MASS),
            hue: generate_random_hue(),
            momentum: 1.,
            last_split: None,
        }
    }

    fn speed(&self) -> f64 {
        // game point per tick
        (INIT_CELL_SPEED / (self.mass.log(LOG_BASE) - INIT_MASS_LOG + 1.)) + self.momentum
    }

    fn split(&mut self) -> Option<PlayerCell> {
        if self.mass < DEFAULT_MASS * 2. {
            return None;
        }
        self.update_mass(self.mass / 2.);
        self.last_split = Some(SystemTime::now());
        Some(PlayerCell {
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

impl CellTrait for FoodCell {}

impl Player {
    fn new_player(addr: SocketAddr, tx: UnboundedSender<Message>, pos: Position) -> Player{
        Player {
            id: generate_player_id(),
            addr: addr,
            tx: tx,
            cells: vec![PlayerCell::new_player_cell(pos)],
            target: None,
            visible_range: MINIMUM_VISIBLE_RANGE,

        }
    }

    fn update_visible_range(&mut self) {
        self.visible_range = 100. * (self.radius() - 22.).max(0.).sqrt() + MINIMUM_VISIBLE_RANGE;

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
        x: rng.gen_range(mass_to_radius(DEFAULT_MASS)..=GAME_WIDTH).floor(),
        y: rng.gen_range(mass_to_radius(DEFAULT_MASS)..=GAME_HEIGHT).floor(),
    }
}

fn distance_between_circles<T, U>(a: &T, b: &U) -> f64
    where T: PositionTrait + RadiusTrait,
          U: PositionTrait + RadiusTrait
{
    a.distance_to(b) - a.radius() - b.radius()
}


fn generate_player_id() -> String {
    Uuid::new_v4().to_string()
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
            let tmp_cell = PlayerCell::new_player_cell(rand_pos);
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

#[derive(Debug, Default)]
pub struct Game {
    players: Players,
    food: Food,
    food_stack: i32,
    peer_map: PeerMap,
    addr_player_id_map: HashMap<SocketAddr, String>,
}


impl Game {
    pub fn new(peer_map: crate::PeerMap) -> Game {
        Game {
            players: HashMap::new(),
            food: Vec::new(),
            food_stack: 0,
            peer_map: peer_map,
            addr_player_id_map: HashMap::new(),
        }
    }

    pub fn add_player(&mut self, addr: SocketAddr) -> Result<(), GameError> {
        if self.addr_player_id_map.contains_key(&addr) {
            return Err(GameError::PlayerAlreadyInGame)
        }

        let tx = self.peer_map.lock().unwrap().get(&addr).unwrap().clone();

        let player = Player::new_player(
            addr,
            tx,
            get_new_player_position(&self.players)
        );
        println!("new player entered the game. Player ID [{}]", player.id);
        self.addr_player_id_map.insert(addr, player.id.clone());
        self.players.insert(player.id.clone(), player);
        self.food_stack += NEW_PLAYER_FOOD;

        Ok(())
    }

    pub fn set_target(&mut self, addr: SocketAddr, x: f64, y: f64) {
        if let Some(player_id) = self.addr_player_id_map.get(&addr) {
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
        if let Some(player_id) = self.addr_player_id_map.get(&addr) {
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

    fn notify_player(&self, player: &Player , method: &str, params: Params) {
        let peer_map = self.peer_map.lock().unwrap();
        let tx = peer_map.get(&player.addr).expect("Missing player peer tx in notify_player");
        tx.unbounded_send(Message::text(
            json!({
                "method": method,
                "params": params,
            }).to_string()
        ));
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
                        let rad = cell_target.y.atan2(cell_target.x);
                        let cell_speed = cell.speed();
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
                        if cell.pos.x > GAME_WIDTH - border_padding {
                            cell.pos.x = GAME_WIDTH - border_padding;
                        }
                        if cell.pos.y > GAME_HEIGHT - border_padding {
                            cell.pos.y = GAME_HEIGHT - border_padding;
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

    fn check_collisions(&mut self) {
        let mut players_vec: Vec<&mut Player> = self.players.values_mut().collect();

        for i in 0..players_vec.len() {
            let (player, mut other_players) = players_vec.split_one_mut(i);

            // handle collision with food cells
            for cell in &mut player.cells {
                self.food.retain(|f| {
                    if cell.is_collide(f) {
                        cell.update_mass(cell.mass + f.mass);
                        return false;
                    }
                    true
                });
            }

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
                self.addr_player_id_map.remove(&player.addr);
                self.notify_game_over(player);
                return false
            }
            true

        });
        self.players = players;

    }

    fn notify_game_over(&self, player: &Player) {
        self.notify_player(player, "notify_game_over", Params::None);
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

    pub fn player_lost_connection(&mut self, addr: SocketAddr) {
        if let Some(player_id) = self.addr_player_id_map.remove(&addr) {
            self.remove_player(&player_id);
        }
    }

    fn remove_player(&mut self, player_id: &str) {
        println!("Removing player. Player ID [{}]", player_id);
        self.players.remove(player_id);
    }

    fn get_state(&self) -> State {
        State {
            players: self.players.clone(),
            food: self.food.clone(),
        }
    }

}

async fn tick_loop(game: crate::Game) {
    let mut sleep_time = 0.;
    loop {
        time::sleep(Duration::from_millis(sleep_time as u64)).await;

        let now = SystemTime::now();
        let mut game = game.lock().unwrap();
        game.move_players();
        game.check_collisions();

        let elapsed = now.elapsed()
            .unwrap_or_default().as_millis() as f64;
        sleep_time = (1000. / TICKS_PER_SEC as f64 - elapsed).max(1.);
    }
}

async fn food_loop(game: crate::Game) {
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
        let state = game.lock().unwrap().get_state();
        for player in state.players.values() {
            let cells: Vec<&PlayerCell> = state.players.values()
                .flat_map(|x| &x.cells)
                .filter(|other_cell| player.is_visible(*other_cell))
                .collect();
            let food: Vec<&FoodCell> = state.food.iter().filter(|f| player.is_visible(*f)).collect();
            let update = json!({
                "method": "update",
                "params": {
                    "position": player.position(),
                    "visible": player.visible_range,
                    "cells": cells,
                    "food": food,
                }
            }).to_string();
            player.tx.unbounded_send(Message::text(update));
        }

        let elapsed = now.elapsed()
            .unwrap_or_default().as_millis() as f64;
        // println!("update {}", elapsed);
        sleep_time = (1000. / UPDATES_PER_SEC as f64 - elapsed).max(1.);
    }
}


pub fn start_tasks(game: crate::Game) {
    tokio::spawn(tick_loop(game.clone()));
    tokio::spawn(food_loop(game.clone()));
    tokio::spawn(update_loop(game.clone()));
}