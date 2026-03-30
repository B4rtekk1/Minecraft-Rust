use super::mob::Mob;
use super::zombie;
use super::villager;

pub fn all_mobs() -> Vec<&'static Mob> {
    vec![
        zombie::ZOMBIE,
        zombie::BABY_ZOMBIE,
        villager::VILLAGER,
        villager::BABY_VILLAGER,
    ]
}