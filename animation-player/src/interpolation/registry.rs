use crate::animation::AnimationTransition;
use crate::interpolation::cache::InterpolationCacheKey;
use crate::interpolation::functions::{
    BezierInterpolation, CubicInterpolation, EaseInInterpolation, EaseInOutInterpolation,
    EaseOutInterpolation, Interpolator, LinearInterpolation, SpringInterpolation,
    StepInterpolation,
};
use crate::interpolation::metrics::InterpolationMetrics;
use crate::time::Timer;
use crate::{AnimationError, Value};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;

/// Registry for managing interpolation functions
pub struct InterpolationRegistry {
    functions: HashMap<String, Box<dyn Interpolator>>,
    cache: LruCache<InterpolationCacheKey, Value>,
    metrics: InterpolationMetrics,
    enable_caching: bool,
    enable_metrics: bool,
}

impl InterpolationRegistry {
    /// Create a new interpolation registry
    #[inline]
    pub fn new(cache_size: usize) -> Self {
        let cache_size = NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(1).unwrap());
        let mut registry = Self {
            functions: HashMap::new(),
            cache: LruCache::new(cache_size),
            metrics: InterpolationMetrics::new(),
            enable_caching: true,
            enable_metrics: true,
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
    ) -> Result<Value, AnimationError> {
        let timer = if self.enable_metrics {
            Some(Timer::new())
        } else {
            None
        };

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
                if let Some(timer) = timer {
                    self.metrics
                        .record_interpolation(timer.elapsed_micros() as u64, true);
                }
                return Ok(cloned_value);
            }
        }

        // Get the function again for interpolation (safe because we checked above)
        let function = self.get_function(function_name).unwrap();

        // Perform interpolation
        let result = function.interpolate(start, end, context)?;

        // Cache result
        if self.enable_caching {
            let cache_key = InterpolationCacheKey::new(interpolation_type, start, end, context.t);
            self.cache.put(cache_key, result.clone());
        }

        // Record metrics
        if let Some(timer) = timer {
            self.metrics
                .record_interpolation(timer.elapsed_micros() as u64, false);
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

    /// Enable or disable metrics
    #[inline]
    pub fn set_metrics_enabled(&mut self, enabled: bool) {
        self.enable_metrics = enabled;
        if !enabled {
            self.metrics.reset();
        }
    }

    /// Get performance metrics
    #[inline]
    pub fn metrics(&self) -> &InterpolationMetrics {
        &self.metrics
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
    ) -> Result<Value, AnimationError> {
        // Create a context with transition parameters only if needed
        let context_with_params = if transition.parameters.is_empty() {
            context.clone()
        } else {
            let mut new_context = context.clone();
            for (key, value) in &transition.parameters {
                if let Ok(param_value) = value.parse::<f64>() {
                    new_context.set_property(key, param_value);
                }
            }
            new_context
        };

        self.interpolate(transition.variant.name(), start, end, &context_with_params)
    }
}

impl Default for InterpolationRegistry {
    fn default() -> Self {
        Self::new(1000) // Default cache size
    }
}
