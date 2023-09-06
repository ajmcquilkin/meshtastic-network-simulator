use rand::Rng;

use super::point::Point;

#[derive(Clone, Debug, Default)]
pub struct Rectangle {
    pub origin: Point,
    pub width: u32,
    pub height: u32,
}

impl Rectangle {
    pub fn get_random_contained_point(&self) -> Point {
        let x = rand::thread_rng().gen_range(self.origin.x..(self.origin.x + self.width));
        let y = rand::thread_rng().gen_range(self.origin.y..(self.origin.y + self.height));

        Point { x, y }
    }

    pub fn contains_point(&self, point: &Point) -> bool {
        point.x >= self.origin.x
            && point.x <= (self.origin.x + self.width)
            && point.y >= self.origin.y
            && point.y <= (self.origin.y + self.height)
    }
}
