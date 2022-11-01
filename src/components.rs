use std::collections::VecDeque;

use bevy::prelude::*;

#[derive(Component, Eq, PartialEq, Hash, Clone, Debug)]
pub struct UserId(pub String);

#[derive(Component)]
pub struct BulletId(pub i32);

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct InterpolationBuffer(pub VecDeque<Transform>);

#[derive(Component)]
pub struct CurrentPlayer;