// FEAT: I could use Rc or Arc to make this Clone
pub struct RefOwner<T: 'static, R> {
    /// WARNING: Field order matters
    /// The target must be declared FIRST so it's dropped LAST
    #[allow(unused)]
    target: Box<T>,
    reference: R,
}

impl<T: 'static + std::fmt::Debug, R: std::fmt::Debug> std::fmt::Debug for RefOwner<T, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.debug_struct("RefOwner")
        // .field("target", &self.target)
        // .field("reference", &self.reference)
        f.debug_tuple("RefOwner").field(&self.reference).finish()
    }
}

impl<T: 'static, R> RefOwner<T, R> {
    pub fn new(target: T, reference: R) -> Self {
        Self {
            target: Box::new(target),
            reference,
        }
    }
}

impl<T, R> std::ops::Deref for RefOwner<T, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.reference
    }
}

pub trait RefTarget<'a, I: 'static + ?Sized>: Sized {
    fn from_ref(target: &'static I) -> Self;
    fn from_owned<T: AsRef<I>>(target: T) -> RefOwner<T, Self> {
        let boxed = Box::new(target);

        /*
        SAFETY ANALYSIS:
        1. Box doesn't need Pin becaause Box heap allocations are stable
        2. The pointer cast extends the lifetime artificially
        3. This is sound because:
            - The Box owns the data and won't move it (heap allocation)
            - We store both Box and reference in RefOwner
            - RefOwner is not Copy/Clone, preventing reference escape
        */
        let reference = unsafe {
            // This cast erases lifetime info
            let target_ref = &*(boxed.as_ref().as_ref() as *const I);
            Self::from_ref(target_ref)
        };
        RefOwner {
            target: boxed,
            reference,
        }
    }
}

pub trait TryRefTarget<'a, I: 'static + ?Sized>: Sized {
    type Err: 'static;
    fn try_from_ref(target: &'a I) -> Result<Self, Self::Err>;
    fn try_from_owned<T: AsRef<I>>(target: T) -> Result<RefOwner<T, Self>, Self::Err> {
        let boxed = Box::new(target);

        /*
        SAFETY ANALYSIS:
        1. Box doesn't need Pin becaause Box heap allocations are stable
        2. The pointer cast extends the lifetime artificially
        3. This is sound because:
            - The Box owns the data and won't move it (heap allocation)
            - We store both Box and reference in RefOwner
            - RefOwner is not Copy/Clone, preventing reference escape
        */
        let reference = unsafe {
            // This cast erases lifetime info
            let target_ref = &*(boxed.as_ref().as_ref() as *const I);
            Self::try_from_ref(target_ref)?
        };
        Ok(RefOwner {
            target: boxed,
            reference,
        })
    }
}
