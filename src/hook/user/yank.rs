pub unsafe trait Yank<T> {
    unsafe fn yank(self) -> T;
    unsafe fn yank_ref(&self) -> &T;
    unsafe fn yank_mut(&mut self) -> &mut T;
}

unsafe impl<T> Yank<T> for Option<T> {
    unsafe fn yank(self) -> T {
        self.unwrap_or_else(|| std::hint::unreachable_unchecked())
    }

    unsafe fn yank_ref(&self) -> &T {
        self.as_ref().yank()
    }

    unsafe fn yank_mut(&mut self) -> &mut T {
        self.as_mut().yank()
    }
}