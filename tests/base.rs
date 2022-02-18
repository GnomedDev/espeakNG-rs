pub fn init<'a>() -> parking_lot::MutexGuard<'a, espeakng::Speaker> {
    espeakng::initialise(None).unwrap().lock()
}
