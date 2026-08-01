use std::cmp::{Eq, PartialEq};
use std::hash::{Hash, Hasher};

pub struct ImplicationWrapper<O, S>
where
    O: Eq + Hash,
{
    pub owner: O,
    pub shadow: S,
}

impl<O, S> ImplicationWrapper<O, S>
where
    O: Eq + Hash,
{
    pub fn new(owner: O, shadow: S) -> ImplicationWrapper<O, S> {
        ImplicationWrapper { owner, shadow }
    }
}

impl<O, S> Hash for ImplicationWrapper<O, S>
where
    O: Hash + Eq,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.owner.hash(state)
    }
}

impl<O, S> PartialEq for ImplicationWrapper<O, S>
where
    O: Hash + Eq,
{
    fn eq(&self, other: &ImplicationWrapper<O, S>) -> bool {
        self.owner.eq(&other.owner)
    }
}

impl<O, S> Eq for ImplicationWrapper<O, S> where O: Hash + Eq {}
