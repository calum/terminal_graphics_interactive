/// A list with a fixed capacity, for bullets, fireballs and particles.
///
/// The storage is allocated once, by [`Pool::with_capacity`], and never grows.
/// A full pool hands the item back from [`push`](Pool::push) instead of
/// allocating, so a game loop never allocates per frame. Fireballs that drift
/// left, dropped once they leave the screen:
///
/// ```
/// use terminal_graphics_interactive::game::Pool;
///
/// struct Fireball {
///     x: f32,
///     y: f32,
/// }
///
/// let mut fireballs = Pool::with_capacity(32);
/// // when the player fires: a full pool just ignores the shot
/// let _ = fireballs.push(Fireball { x: 70.0, y: 12.0 });
///
/// // every update: move them, then drop the ones that left the screen
/// for fireball in &mut fireballs {
///     fireball.x -= 1.5;
/// }
/// fireballs.retain(|f| f.x >= 0.0);
/// assert_eq!(fireballs.len(), 1);
/// assert_eq!(fireballs.get(0).map(|f| f.y), Some(12.0));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Pool<T> {
    items: Vec<T>,
    limit: usize,
}

impl<T> Pool<T> {
    /// An empty pool that holds up to `capacity` items.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool: Pool<u8> = Pool::with_capacity(2);
    /// assert!(pool.is_empty());
    /// assert_eq!(pool.capacity(), 2);
    /// ```
    pub fn with_capacity(capacity: usize) -> Pool<T> {
        Pool {
            items: Vec::with_capacity(capacity),
            limit: capacity,
        }
    }

    /// Add an item. A full pool gives the item back as `Err`.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(1);
    /// assert_eq!(pool.push("first"), Ok(()));
    /// assert_eq!(pool.push("second"), Err("second")); // full: the item comes back
    /// ```
    pub fn push(&mut self, item: T) -> Result<(), T> {
        if self.items.len() < self.limit {
            self.items.push(item);
            Ok(())
        } else {
            Err(item)
        }
    }

    /// Number of items in the pool.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(4);
    /// pool.push(1).unwrap();
    /// assert_eq!(pool.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Remove every item. The capacity stays the same.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(2);
    /// pool.push(1).unwrap();
    /// pool.clear();
    /// assert!(pool.is_empty());
    /// assert_eq!(pool.capacity(), 2);
    /// ```
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// True if the pool holds no items.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let pool: Pool<u8> = Pool::with_capacity(4);
    /// assert!(pool.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// True if no more items fit.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(1);
    /// pool.push(1).unwrap();
    /// assert!(pool.is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.limit
    }

    /// The most items the pool holds. This never changes.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let pool: Pool<u8> = Pool::with_capacity(64);
    /// assert_eq!(pool.capacity(), 64);
    /// ```
    pub fn capacity(&self) -> usize {
        self.limit
    }

    /// Keep only the items for which `keep` returns true. The order of the
    /// rest is unchanged, and the capacity is unchanged.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(4);
    /// for n in [1, 2, 3, 4] {
    ///     pool.push(n).unwrap();
    /// }
    /// pool.retain(|&n| n % 2 == 0);
    /// assert_eq!(pool.len(), 2);
    /// ```
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, keep: F) {
        self.items.retain(keep);
    }

    /// The item at `index` (0 is the oldest), or `None` past the end.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(2);
    /// pool.push("x").unwrap();
    /// assert_eq!(pool.get(0), Some(&"x"));
    /// assert_eq!(pool.get(1), None);
    /// ```
    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    /// Mutable access to the item at `index`, or `None` past the end.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(2);
    /// pool.push(5).unwrap();
    /// if let Some(v) = pool.get_mut(0) {
    ///     *v += 1;
    /// }
    /// assert_eq!(pool.get(0), Some(&6));
    /// ```
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(index)
    }

    /// The items, oldest first.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(3);
    /// pool.push('a').unwrap();
    /// pool.push('b').unwrap();
    /// let s: String = pool.iter().collect();
    /// assert_eq!(s, "ab");
    /// ```
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// The items, oldest first, that can be changed in place.
    ///
    /// ```
    /// use terminal_graphics_interactive::game::Pool;
    ///
    /// let mut pool = Pool::with_capacity(2);
    /// pool.push(1).unwrap();
    /// pool.push(2).unwrap();
    /// for x in pool.iter_mut() {
    ///     *x *= 10;
    /// }
    /// assert_eq!(pool.get(1), Some(&20));
    /// ```
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.items.iter_mut()
    }
}

impl<'a, T> IntoIterator for &'a Pool<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Pool<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_fills_to_capacity_then_hands_items_back() {
        let mut pool = Pool::with_capacity(3);
        assert_eq!(pool.push(1), Ok(()));
        assert_eq!(pool.push(2), Ok(()));
        assert_eq!(pool.push(3), Ok(()));
        assert!(pool.is_full());
        assert_eq!(pool.push(4), Err(4));
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn retain_keeps_the_survivors_in_their_order() {
        let mut pool = Pool::with_capacity(6);
        for i in [5, 2, 8, 1, 9, 4] {
            pool.push(i).unwrap();
        }
        pool.retain(|&x| x > 3);
        let kept: Vec<i32> = pool.iter().copied().collect();
        assert_eq!(kept, vec![5, 8, 9, 4]);
    }

    #[test]
    fn a_zero_capacity_pool_is_always_full() {
        let mut pool: Pool<u8> = Pool::with_capacity(0);
        assert!(pool.is_full());
        assert!(pool.is_empty());
        assert_eq!(pool.push(1), Err(1));
        pool.retain(|_| true);
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn clear_empties_the_pool_but_keeps_its_capacity() {
        let mut pool = Pool::with_capacity(2);
        pool.push(1).unwrap();
        pool.push(2).unwrap();
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.capacity(), 2);
        assert_eq!(pool.push(3), Ok(()));
        assert_eq!(pool.push(4), Ok(()));
        assert_eq!(pool.push(5), Err(5));
    }

    #[test]
    fn get_and_get_mut_return_none_past_the_end() {
        let mut pool = Pool::with_capacity(2);
        pool.push('a').unwrap();
        assert_eq!(pool.get(0), Some(&'a'));
        assert_eq!(pool.get(1), None);
        *pool.get_mut(0).unwrap() = 'b';
        assert_eq!(pool.get(0), Some(&'b'));
        assert!(pool.get_mut(5).is_none());
    }

    #[test]
    fn a_pool_can_be_looped_over_by_reference() {
        let mut pool = Pool::with_capacity(3);
        pool.push(10).unwrap();
        pool.push(20).unwrap();
        let mut total = 0;
        for x in &pool {
            total += x;
        }
        assert_eq!(total, 30);
        for x in &mut pool {
            *x += 1;
        }
        assert_eq!(pool.iter().copied().collect::<Vec<_>>(), vec![11, 21]);
    }

    #[test]
    fn many_push_and_retain_cycles_never_reallocate() {
        let mut pool: Pool<u32> = Pool::with_capacity(100);
        let storage_before = pool.items.capacity();
        for i in 0..10_000 {
            if pool.push(i).is_err() {
                pool.retain(|x| x % 3 != 0);
            }
        }
        assert_eq!(pool.capacity(), 100);
        assert_eq!(pool.items.capacity(), storage_before);
    }
}
