
use std::collections::HashMap;
use serde::{Serialize};


use crate::game::{
    Position,
    PositionTrait,
};

type Cells<T> = HashMap<(u32, u32), Vec<T>>;
type SerializedCells = HashMap<(u32, u32), String>;

pub struct Grid<'a, T> {
    grid_size: u32,
    cell_size: u32,
    cells: Cells<&'a T>,
    serialized_cells: SerializedCells,
}

impl<'a, T> Grid<'a, T>
    where T: PositionTrait + Serialize {

    pub fn new(size: u32, cell_size: u32, items: impl Iterator<Item=&'a T>) -> Grid<'a, T> {
        let grid_size = size / cell_size;
        let mut cells = HashMap::new();
        for item in items {
            let Position {x , y} = item.position();
            let ix = x as u32 / cell_size;
            let iy = y as u32 / cell_size;
            let key = (ix, iy);
            cells.entry(key).or_insert_with(Vec::new).push(item);
        }
        Grid {
            grid_size: grid_size,
            cell_size: cell_size,
            cells: cells,
            serialized_cells: SerializedCells::new(),
        }

    }

    pub fn query(&'a self, position: Position, range: u32) -> impl Iterator<Item=&'a T> {
        let x = position.x as u32 / self.cell_size;
        let y = position.y as u32 / self.cell_size;
        let mut values = Vec::new();
        let range = range / self.cell_size;
        let x_range = (x - range.min(x))..=(x + range).min(self.grid_size);
        let y_range = (y - range.min(y))..=(y + range).min(self.grid_size);
        for x_idx in x_range {
            for y_idx in y_range.clone() {
                let key = (x_idx, y_idx);
                if let Some(value) = self.cells.get(&key) {
                    values.push(value);
                }
            }
        }
        values.into_iter().flatten().map(|x| &**x).into_iter()
    }

    pub fn query_serialized(&mut self, position: Position, range: u32) -> Vec<String> {
        let x = position.x as u32 / self.cell_size;
        let y = position.y as u32 / self.cell_size;
        let range = range / self.cell_size;
        let mut values = Vec::new();
        let x_range = (x - range.min(x))..=(x + range).min(self.grid_size);
        let y_range = (y - range.min(y))..=(y + range).min(self.grid_size);
        let cells = std::mem::take(&mut self.cells);
        for x_idx in x_range.clone() {
            for y_idx in y_range.clone() {
                let key = (x_idx as u32, y_idx as u32);
                if let Some(data) = self.serialized_cells.get(&key) {
                    values.push(data.clone());
                } else {
                    if let Some(data) = cells.get(&key) {
                        if let Ok(data_str) = serde_json::to_string(data) {
                            values.push(data_str.clone());
                            self.serialized_cells.insert(key, data_str);
                        }
                    }
                }
            }
        }
        self.cells = cells;
        values
    }


}
