use crate::animation::AnimationTransition;
use crate::interpolation::cache::InterpolationCacheKey;
use crate::interpolation::functions::{
    BezierInterpolation, CatmullRomInterpolation, CubicInterpolation, EaseInInterpolation,
    EaseInOutInterpolation, EaseOutInterpolation, HermiteInterpolation, Interpolator,
    LinearInterpolation, SpringInterpolation, StepInterpolation, BSplineInterpolation
};
use crate::interpolation::parameters::InterpolationParams;
use crate::{AnimationError, Value};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;

/// Registry for managing interpolation functions
pub struct InterpolationRegistry {
    functions: HashMap<String, Box<dyn Interpolator>>,
    cache: LruCache<InterpolationCacheKey, Value>,
    enable_caching: bool,
}

impl InterpolationRegistry {
    /// Create a new interpolation registry
    #[inline]
    pub fn new(cache_size: usize) -> Self {
        let cache_size = NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1).unwrap());
        let mut registry = Self {
            functions: HashMap::new(),
            cache: LruCache::new(cache_size),
            enable_caching: true,
        };

        // Register built-in interpolation functions
        registry.register_builtin_functions();
        registry
    }

    /// Register built-in interpolation functions
    #[inline]
    fn register_builtin_functions(&mut self) {
        self.register_function(Box::new(LinearInterpolation));
        self.register_function(Box::new(CubicInterpolation));
        self.register_function(Box::new(StepInterpolation));
        self.register_function(Box::new(BezierInterpolation::new()));
        self.register_function(Box::new(SpringInterpolation::new()));
        self.register_function(Box::new(CatmullRomInterpolation));
        self.register_function(Box::new(HermiteInterpolation));
        self.register_function(Box::new(BSplineInterpolation));

        // Add common easing functions
        self.register_function(Box::new(EaseInInterpolation));
        self.register_function(Box::new(EaseOutInterpolation));
        self.register_function(Box::new(EaseInOutInterpolation));
    }

    /// Register a new interpolation function
    #[inline]
    pub fn register_function(&mut self, function: Box<dyn Interpolator>) {
        self.functions.insert(function.name().to_string(), function);
    }

    /// Get an interpolation function by name
    #[inline]
    pub fn get_function(&self, name: &str) -> Option<&dyn Interpolator> {
        self.functions.get(name).map(|f| f.as_ref())
    }

    /// List all available interpolation functions
    #[inline]
    pub fn list_functions(&self) -> Vec<&str> {
        self.functions.keys().map(|k| k.as_str()).collect()
    }

    /// Perform interpolation
    pub fn interpolate(
        &mut self,
        function_name: &str,
        start: &Value,
        end: &Value,
        context: &crate::interpolation::context::InterpolationContext,
        animation: &crate::AnimationData,
    ) -> Result<Value, AnimationError> {
        // First, check if the function exists and get its interpolation type
        let interpolation_type = {
            let function = self.get_function(function_name).ok_or_else(|| {
                AnimationError::InterpolationNotFound {
                    name: function_name.to_string(),
                }
            })?;
            function.interpolation_type()
        };

        // Check cache first
        if self.enable_caching {
            let cache_key = InterpolationCacheKey::new(interpolation_type, start, end, context.t);

            if let Some(cached_value) = self.cache.get(&cache_key) {
                let cloned_value = cached_value.clone();
                return Ok(cloned_value);
            }
        }

        // Get the function again for interpolation (safe because we checked above)
        let function = self.get_function(function_name).unwrap();

        // Perform interpolation
        let result = function.interpolate(start, end, context, animation)?;

        // Cache result
        if self.enable_caching {
            let cache_key = InterpolationCacheKey::new(interpolation_type, start, end, context.t);
            self.cache.put(cache_key, result.clone());
        }

        Ok(result)
    }

    /// Enable or disable caching
    #[inline]
    pub fn set_caching_enabled(&mut self, enabled: bool) {
        self.enable_caching = enabled;
        if !enabled {
            self.cache.clear();
        }
    }

    /// Clear the interpolation cache
    #[inline]
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics
    #[inline]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Get cache capacity
    #[inline]
    pub fn cache_cap(&self) -> usize {
        self.cache.cap().into()
    }

    /// Perform interpolation using a transition
    pub fn interpolate_with_transition(
        &mut self,
        transition: &AnimationTransition,
        start: &Value,
        end: &Value,
        context: &crate::interpolation::context::InterpolationContext,
        animation: &crate::AnimationData,
    ) -> Result<Value, AnimationError> {
        // Create a context with transition parameters only if needed
        let context_with_params = if let Some(params) = &transition.parameters {
            let mut new_context = context.clone();
            match params {
                InterpolationParams::Spring(p) => {
                    new_context.set_property("mass", p.mass as f64);
                    new_context.set_property("stiffness", p.stiffness as f64);
                    new_context.set_property("damping", p.damping as f64);
                }
                InterpolationParams::Bezier(p) => {
                    new_context.set_property("x1", p.x1 as f64);
                    new_context.set_property("y1", p.y1 as f64);
                    new_context.set_property("x2", p.x2 as f64);
                    new_context.set_property("y2", p.y2 as f64);
                }
                InterpolationParams::Step(p) => {
                    new_context.set_property("threshold", p.point as f64);
                }
                InterpolationParams::Hermite(p) => {
                    if let Some(val) = &p.tangent_start {
                        new_context.set_property("tangent_start", val.clone());
                    }
                    if let Some(val) = &p.tangent_end {
                        new_context.set_property("tangent_end", val.clone());
                    }
                }
            }
            new_context
        } else {
            context.clone()
        };

        self.interpolate(
            transition.variant.name(),
            start,
            end,
            &context_with_params,
            animation,
        )
    }
}

impl Default for InterpolationRegistry {
    fn default() -> Self {
        Self::new(1000) // Default cache size
    }
}
