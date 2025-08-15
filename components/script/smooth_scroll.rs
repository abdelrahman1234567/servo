/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use constellation_traits::ScriptToConstellationMessage;
use embedder_traits::AnimationState as AnimationsPresentState;
use euclid::default::Point2D;
use webrender_api::ExternalScrollId;

use crate::dom::bindings::cell::DomRefCell;
use crate::dom::bindings::root::DomRoot;
use crate::dom::element::Element;
use crate::dom::window::Window;

/// Duration for smooth scroll animations in milliseconds
const SMOOTH_SCROLL_DURATION_MS: u64 = 150;

/// The animation curve used for smooth scrolling
const SCROLL_TIMING_FUNCTION: CubicBezier = CubicBezier {
    x1: 0.25,
    y1: 0.1,
    x2: 0.25,
    y2: 1.0,
};

/// A cubic bezier timing function for animations
#[derive(Clone, Copy, Debug)]
struct CubicBezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

// TODO: Implement Web Animations APIs and move CubicBezier there
impl CubicBezier {
    /// Evaluate the cubic bezier curve at time `t` (0.0 to 1.0)
    fn evaluate(&self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }

        // Use Newton-Raphson method to solve for the curve value
        let mut x = t;
        for _ in 0..8 {
            let curve_x = self.curve_x(x);
            let curve_x_derivative = self.curve_x_derivative(x);

            if curve_x_derivative.abs() < 1e-6 {
                break;
            }

            x -= (curve_x - t) / curve_x_derivative;
        }

        self.curve_y(x.clamp(0.0, 1.0))
    }

    fn curve_x(&self, t: f32) -> f32 {
        3.0 * (1.0 - t) * (1.0 - t) * t * self.x1 + 3.0 * (1.0 - t) * t * t * self.x2 + t * t * t
    }

    fn curve_y(&self, t: f32) -> f32 {
        3.0 * (1.0 - t) * (1.0 - t) * t * self.y1 + 3.0 * (1.0 - t) * t * t * self.y2 + t * t * t
    }

    fn curve_x_derivative(&self, t: f32) -> f32 {
        3.0 * (1.0 - t) * (1.0 - t) * self.x1 +
            6.0 * (1.0 - t) * t * (self.x2 - self.x1) +
            3.0 * t * t * (1.0 - self.x2)
    }
}

#[derive(Debug)]
pub(crate) struct SmoothScrollAnimation {
    /// The element being scrolled
    #[allow(dead_code)] // Used for finding elements in find_element_for_scroll_id
    target: Option<DomRoot<Element>>,

    /// The external scroll ID for communicating with the compositor
    #[allow(dead_code)] // Used as HashMap key, not directly accessed
    scroll_id: ExternalScrollId,

    /// Starting scroll position
    start_position: Point2D<f32>,

    /// Target scroll position
    target_position: Point2D<f32>,

    /// When the animation started
    start_time: Instant,

    /// Duration of the animation
    duration: Duration,

    /// Whether the animation is still running
    is_running: bool,
}

impl SmoothScrollAnimation {
    pub(crate) fn new(
        target: Option<&Element>,
        scroll_id: ExternalScrollId,
        start_position: Point2D<f32>,
        target_position: Point2D<f32>,
    ) -> Self {
        Self {
            target: target.map(DomRoot::from_ref),
            scroll_id,
            start_position,
            target_position,
            start_time: Instant::now(),
            duration: Duration::from_millis(SMOOTH_SCROLL_DURATION_MS),
            is_running: true,
        }
    }

    /// Update the animation and return the current scroll position
    pub(crate) fn update(&mut self, now: Instant) -> Option<Point2D<f32>> {
        if !self.is_running {
            return None;
        }

        let elapsed = now.duration_since(self.start_time);
        if elapsed >= self.duration {
            self.is_running = false;
            return Some(self.target_position);
        }

        let progress = elapsed.as_millis() as f32 / self.duration.as_millis() as f32;
        let eased_progress = SCROLL_TIMING_FUNCTION.evaluate(progress);

        let current_x = self.start_position.x +
            (self.target_position.x - self.start_position.x) * eased_progress;
        let current_y = self.start_position.y +
            (self.target_position.y - self.start_position.y) * eased_progress;

        let current_position = Point2D::new(current_x, current_y);

        Some(current_position)
    }

    /// Check if the animation is still running
    pub(crate) fn is_running(&self) -> bool {
        self.is_running
    }

    /// Get the target position for this animation
    #[allow(dead_code)]
    pub(crate) fn target_position(&self) -> Point2D<f32> {
        self.target_position
    }

    /// Get the target element for this animation
    #[allow(dead_code)]
    pub(crate) fn target_element(&self) -> Option<&DomRoot<Element>> {
        self.target.as_ref()
    }
}

/// Manages all smooth scroll animations for a document
#[derive(Default)]
pub(crate) struct SmoothScrollAnimations {
    /// Active smooth scroll animations, keyed by scroll ID
    animations: DomRefCell<HashMap<ExternalScrollId, SmoothScrollAnimation>>,

    /// Whether we have any running smooth scroll animations
    has_running_animations: Cell<bool>,
}

// Implement JSTraceable manually to avoid complex derives
#[allow(unsafe_code)]
unsafe impl crate::dom::bindings::trace::JSTraceable for SmoothScrollAnimations {
    #[allow(unsafe_code)]
    unsafe fn trace(&self, _tracer: *mut js::jsapi::JSTracer) {
        // SmoothScrollAnimations doesn't contain any JS objects that need tracing
    }
}

// Manual MallocSizeOf implementation to handle the complex types
impl malloc_size_of::MallocSizeOf for SmoothScrollAnimations {
    fn size_of(&self, _ops: &mut malloc_size_of::MallocSizeOfOps) -> usize {
        // Simplified size calculation - the actual animations are complex to measure
        0
    }
}

impl malloc_size_of::MallocSizeOf for SmoothScrollAnimation {
    fn size_of(&self, _ops: &mut malloc_size_of::MallocSizeOfOps) -> usize {
        // Basic size calculation for animation struct
        std::mem::size_of::<Self>()
    }
}

impl SmoothScrollAnimations {
    pub(crate) fn new() -> Self {
        Self {
            animations: DomRefCell::new(HashMap::new()),
            has_running_animations: Cell::new(false),
        }
    }

    /// Start a new smooth scroll animation, canceling any existing one for the same scroll ID
    pub(crate) fn start_scroll_animation(
        &self,
        target: Option<&Element>,
        scroll_id: ExternalScrollId,
        start_position: Point2D<f32>,
        target_position: Point2D<f32>,
        window: &Window,
    ) {
        if let Ok(mut animations) = self.animations.try_borrow_mut() {
            let animation =
                SmoothScrollAnimation::new(target, scroll_id, start_position, target_position);

            let had_animations = !animations.is_empty();
            animations.insert(scroll_id, animation);
            let has_animations = !animations.is_empty();

            self.has_running_animations.set(has_animations);
            if !had_animations && has_animations {
                // Started new animations - notify constellation
                self.notify_animation_state_change(window);
            }
        }
    }

    /// Update all running animations and return positions that need to be applied
    pub(crate) fn update_animations(
        &self,
        window: &Window,
    ) -> Vec<(ExternalScrollId, Point2D<f32>, bool)> {
        let now = Instant::now();
        let Ok(mut animations) = self.animations.try_borrow_mut() else {
            return Vec::new();
        };
        let mut positions_to_apply = Vec::new();
        let mut animations_to_remove = Vec::new();

        for (scroll_id, animation) in animations.iter_mut() {
            if let Some(position) = animation.update(now) {
                positions_to_apply.push((*scroll_id, position, animation.is_running()));
                if !animation.is_running() {
                    animations_to_remove.push(*scroll_id);
                }
            } else {
                // Animation was cancelled or completed
                animations_to_remove.push(*scroll_id);
            }
        }

        // Remove completed animations
        for scroll_id in animations_to_remove {
            animations.remove(&scroll_id);
        }

        let had_animations = self.has_running_animations.get();
        let has_animations = !animations.is_empty();

        self.has_running_animations.set(has_animations);

        if had_animations && !has_animations {
            // All animations finished - notify constellation
            self.notify_animation_state_change(window);
        }
        positions_to_apply
    }

    /// Find the element associated with a scroll ID.
    pub(crate) fn find_element_for_scroll_id(
        &self,
        scroll_id: ExternalScrollId,
    ) -> Option<DomRoot<Element>> {
        if let Ok(animations) = self.animations.try_borrow() {
            if let Some(animation) = animations.get(&scroll_id) {
                if let Some(target_element) = animation.target_element() {
                    return Some(DomRoot::from_ref(target_element));
                }
            }
        }
        None
    }

    /// Check if we have any running smooth scroll animations.
    pub(crate) fn has_running_animations(&self) -> bool {
        self.has_running_animations.get()
    }

    /// Check if there's a running animation for the given scroll ID.
    pub(crate) fn has_animation_for_scroll_id(&self, scroll_id: ExternalScrollId) -> bool {
        if let Ok(animations) = self.animations.try_borrow() {
            animations
                .get(&scroll_id)
                .map(|animation| animation.is_running())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Get the target position of a running animation for the given scroll ID.
    pub(crate) fn get_animation_target_position(
        &self,
        scroll_id: ExternalScrollId,
    ) -> Option<Point2D<f32>> {
        if let Ok(animations) = self.animations.try_borrow() {
            animations
                .get(&scroll_id)
                .filter(|animation| animation.is_running())
                .map(|animation| animation.target_position)
        } else {
            None
        }
    }

    /// Check if there's an animation for the scroll ID and cancel it if the target position is different.
    /// Returns true if an animation was found and cancelled.
    pub(crate) fn check_and_cancel_if_target_changed(
        &self,
        scroll_id: ExternalScrollId,
        new_target: Point2D<f32>,
        window: &Window,
    ) {
        if let Ok(mut animations) = self.animations.try_borrow_mut() {
            if let Some(animation) = animations.get(&scroll_id) {
                if animation.is_running() {
                    if animation.target_position.x == new_target.x &&
                        animation.target_position.y == new_target.y
                    {
                        return;
                    }

                    let had_animations = !animations.is_empty();
                    animations.remove(&scroll_id);
                    let has_animations = !animations.is_empty();
                    self.has_running_animations.set(has_animations);

                    if had_animations && !has_animations {
                        // All animations finished - notify constellation
                        self.notify_animation_state_change(window);
                    }
                    return;
                }
            }
        }
    }

    /// Notify the window about animation state changes.
    fn notify_animation_state_change(&self, window: &Window) {
        let state = if self.has_running_animations.get() {
            AnimationsPresentState::AnimationsPresent
        } else {
            AnimationsPresentState::NoAnimationsPresent
        };

        window.send_to_constellation(ScriptToConstellationMessage::ChangeRunningAnimationsState(
            state,
        ));
    }
}
