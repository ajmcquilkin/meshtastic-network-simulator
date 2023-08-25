#[derive(Clone, Debug)]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub is_router: bool,
    pub is_repeater: bool,
    pub hop_limit: u8, // 3 bytes
    pub antenna_gain: f32,
}
