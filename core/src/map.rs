use std::default::{Default};
use std::iter::{repeat};
use cgmath::{Vector2, Array};
use types::{Size2};
use dir::{Dir, DirIter, dirs};
use position::{MapPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance{pub n: i32}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Plain,
    Trees,
    City,
    Water,
}

impl Default for Terrain {
    fn default() -> Terrain { Terrain::Plain }
}

#[derive(Clone, Debug)]
pub struct Map<T> {
    tiles: Vec<T>,
    size: Size2,
}

impl<T: Clone + Default> Map<T> {
    pub fn new(size: Size2) -> Map<T> {
        let tiles_count = (size.w * size.h) as usize;
        let tiles = repeat(Default::default()).take(tiles_count).collect();
        Map {
            tiles: tiles,
            size: size,
        }
    }
}

// TODO: Add some tests
impl<T> Map<T> {
    pub fn from_callback(size: Size2, cb: &mut FnMut(MapPos) -> T) -> Map<T> {
        let tiles_count = (size.w * size.h) as usize;
        let mut tiles = Vec::with_capacity(tiles_count);
        for pos in MapPosIter::new(size) {
            tiles.push(cb(pos));
        }
        Map{tiles, size}
    }
}

impl<T: Clone> Map<T> {
    pub fn size(&self) -> Size2 {
        self.size
    }

    pub fn tile_mut<P: Into<MapPos>>(&mut self, pos: P) -> &mut T {
        let pos = pos.into();
        assert!(self.is_inboard(pos));
        let index = self.size.w * pos.v.y + pos.v.x;
        &mut self.tiles[index as usize]
    }

    pub fn tile<P: Into<MapPos>>(&self, pos: P) -> &T {
        let pos = pos.into();
        assert!(self.is_inboard(pos));
        let index = self.size.w * pos.v.y + pos.v.x;
        &self.tiles[index as usize]
    }

    pub fn is_inboard<P: Into<MapPos>>(&self, pos: P) -> bool {
        let pos = pos.into();
        let x = pos.v.x;
        let y = pos.v.y;
        x >= 0 && y >= 0 && x < self.size.w && y < self.size.h
    }

    pub fn get_iter(&self) -> MapPosIter {
        MapPosIter::new(self.size())
    }
}

#[derive(Clone, Debug)]
pub struct MapPosIter {
    cursor: MapPos,
    map_size: Size2,
}

impl MapPosIter {
    fn new(map_size: Size2) -> MapPosIter {
        MapPosIter {
            cursor: MapPos{v: Vector2::from_value(0)},
            map_size: map_size,
        }
    }
}

impl Iterator for MapPosIter {
    type Item = MapPos;

    fn next(&mut self) -> Option<MapPos> {
        let current_pos = if self.cursor.v.y >= self.map_size.h {
            None
        } else {
            Some(self.cursor)
        };
        self.cursor.v.x += 1;
        if self.cursor.v.x >= self.map_size.w {
            self.cursor.v.x = 0;
            self.cursor.v.y += 1;
        }
        current_pos
    }
}

#[derive(Clone, Debug)]
pub struct RingIter {
    cursor: MapPos,
    segment_index: i32,
    dir_iter: DirIter,
    radius: Distance,
    dir: Dir,
}

pub fn ring_iter(pos: MapPos, radius: Distance) -> RingIter {
    let mut pos = pos;
    pos.v.x -= radius.n;
    let mut dir_iter = dirs();
    let dir = dir_iter.next()
        .expect("Can`t get first direction");
    assert_eq!(dir, Dir::SouthEast);
    RingIter {
        cursor: pos,
        radius: radius,
        segment_index: 0,
        dir_iter: dir_iter,
        dir: dir,
    }
}

impl RingIter {
    fn simple_step(&mut self) -> Option<MapPos> {
        self.cursor = Dir::get_neighbour_pos(
            self.cursor, self.dir);
        self.segment_index += 1;
        Some(self.cursor)
    }

    fn rotate(&mut self, dir: Dir) -> Option<MapPos> {
        self.segment_index = 0;
        self.cursor = Dir::get_neighbour_pos(self.cursor, self.dir);
        self.dir = dir;
        Some(self.cursor)
    }
}

impl Iterator for RingIter {
    type Item = MapPos;

    fn next(&mut self) -> Option<MapPos> {
        if self.segment_index >= self.radius.n - 1 {
            if let Some(dir) = self.dir_iter.next() {
                self.rotate(dir)
            } else if self.segment_index == self.radius.n {
                None
            } else {
                // last pos
                self.simple_step()
            }
        } else {
            self.simple_step()
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpiralIter {
    ring_iter: RingIter,
    radius: Distance,
    last_radius: Distance,
    origin: MapPos,
}

pub fn spiral_iter(pos: MapPos, radius: Distance) -> SpiralIter {
    assert!(radius.n >= 1);
    SpiralIter {
        ring_iter: ring_iter(pos, Distance{n: 1}),
        radius: Distance{n: 1},
        last_radius: radius,
        origin: pos,
    }
}

impl Iterator for SpiralIter {
    type Item = MapPos;

    fn next(&mut self) -> Option<MapPos> {
        let pos = self.ring_iter.next();
        if pos.is_some() {
            pos
        } else {
            self.radius.n += 1;
            if self.radius > self.last_radius {
                None
            } else {
                self.ring_iter = ring_iter(
                    self.origin, self.radius);
                self.ring_iter.next()
            }
        }
    }
}

pub fn distance(from: MapPos, to: MapPos) -> Distance {
    let to = to.v;
    let from = from.v;
    let dx = (to.x + to.y / 2) - (from.x + from.y / 2);
    let dy = to.y - from.y;
    Distance{n: (dx.abs() + dy.abs() + (dx - dy).abs()) / 2}
}

#[cfg(test)]
mod tests {
    use cgmath::{Vector2};
    use map::{MapPos, Distance, ring_iter, spiral_iter};

    #[test]
    fn test_ring_1() {
        let radius = Distance{n: 1};
        let start_pos = MapPos{v: Vector2{x: 0, y: 0}};
        let expected = [
            (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 0) ];
        let mut expected = expected.iter();
        for p in ring_iter(start_pos, radius) {
            let expected = expected.next().expect(
                "Can not get next element from expected vector");
            assert_eq!(*expected, (p.v.x, p.v.y));
        }
        assert!(expected.next().is_none());
    }

    #[test]
    fn test_ring_2() {
        let radius = Distance{n: 2};
        let start_pos = MapPos{v: Vector2{x: 0, y: 0}};
        let expected = [
            (-1, -1),
            (-1, -2),
            (0, -2),
            (1, -2),
            (2, -1),
            (2, 0),
            (2, 1),
            (1, 2),
            (0, 2),
            (-1, 2),
            (-1, 1),
            (-2, 0),
        ];
        let mut expected = expected.iter();
        for p in ring_iter(start_pos, radius) {
            let expected = expected.next().expect(
                "Can not get next element from expected vector");
            assert_eq!(*expected, (p.v.x, p.v.y));
        }
        assert!(expected.next().is_none());
    }

    #[test]
    fn test_spiral_1() {
        let radius = Distance{n: 2};
        let start_pos = MapPos{v: Vector2{x: 0, y: 0}};
        let expected = [
            // ring 1
            (0, -1),
            (1, -1),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 0),
            // ring 2
            (-1, -1),
            (-1, -2),
            (0, -2),
            (1, -2),
            (2, -1),
            (2, 0),
            (2, 1),
            (1, 2),
            (0, 2),
            (-1, 2),
            (-1, 1),
            (-2, 0),
        ];
        let mut expected = expected.iter();
        for p in spiral_iter(start_pos, radius) {
            let expected = expected.next().expect(
                "Can not get next element from expected vector");
            assert_eq!(*expected, (p.v.x, p.v.y));
        }
        assert!(expected.next().is_none());
    }
}
