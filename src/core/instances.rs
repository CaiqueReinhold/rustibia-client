use bevy::prelude::*;

#[derive(Resource, Default, Debug)]
pub struct InstanceManager<T: Default> {
    free_list: Vec<u32>,
    data: Vec<T>,
    dirty: bool,
}

impl<T: Default> InstanceManager<T> {
    pub fn alloc_index(&mut self) -> u32 {
        self.dirty = true;
        if let Some(index) = self.free_list.pop() {
            self.data[index as usize] = T::default();
            index
        } else {
            let index = self.data.len() as u32;
            self.data.push(T::default());
            index
        }
    }

    pub fn dealloc_index(&mut self, index: u32) {
        self.free_list.push(index);
    }

    /// Unconditionally marks the buffer dirty. For initialization, where the
    /// slot is being filled for the first time; per-frame writers should use
    /// [`Self::update`] instead.
    pub fn get_mut(&mut self, index: u32) -> &mut T {
        self.dirty = true;
        &mut self.data[index as usize]
    }

    /// Writes through `f` and marks the buffer dirty only if the instance
    /// actually changed.
    ///
    /// The per-frame update systems re-derive an instance from components that
    /// mostly hold still — a sprite whose animation phase has not advanced
    /// writes the same bytes back. Flagging that as dirty costs a full re-upload
    /// of the whole SSBO on every frame, so the comparison pays for itself many
    /// times over.
    pub fn update<F>(&mut self, index: u32, f: F)
    where
        T: Copy + PartialEq,
        F: FnOnce(&mut T),
    {
        let slot = &mut self.data[index as usize];
        let before = *slot;
        f(slot);
        if *slot != before {
            self.dirty = true;
        }
    }

    pub fn get_buffer_data(&self) -> &[T] {
        &self.data
    }

    pub fn reset_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Clone, Copy, PartialEq, Debug)]
    struct Instance {
        value: u32,
    }

    fn manager() -> InstanceManager<Instance> {
        let mut manager = InstanceManager::<Instance>::default();
        manager.alloc_index();
        manager.reset_dirty();
        manager
    }

    #[test]
    fn a_write_that_changes_nothing_leaves_the_buffer_clean() {
        let mut manager = manager();
        manager.update(0, |instance| instance.value = 0);

        assert!(
            !manager.is_dirty(),
            "an unchanged instance must not trigger an upload"
        );
    }

    #[test]
    fn a_write_that_changes_the_data_dirties_the_buffer() {
        let mut manager = manager();
        manager.update(0, |instance| instance.value = 7);

        assert!(manager.is_dirty());
        assert_eq!(manager.get_buffer_data()[0].value, 7);
    }

    /// Allocation has to dirty unconditionally: the new slot is part of the
    /// buffer whether or not anyone writes to it.
    #[test]
    fn allocating_dirties_the_buffer() {
        let mut manager = manager();
        manager.alloc_index();

        assert!(manager.is_dirty());
    }

    #[test]
    fn a_reused_slot_comes_back_default() {
        let mut manager = manager();
        manager.update(0, |instance| instance.value = 7);
        manager.dealloc_index(0);

        let index = manager.alloc_index();

        assert_eq!(index, 0);
        assert_eq!(manager.get_buffer_data()[0], Instance::default());
    }
}
