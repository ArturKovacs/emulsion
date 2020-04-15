
use std::cell::RefCell;
use std::rc::Rc;

use glium::glutin::{
    self,
    event::{ElementState, MouseButton},
};
use glium::{uniform, Frame, Surface};

use crate::window::Window;
use crate::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement, PickDimension, HorDim, VerDim};
use crate::{add_common_widget_functions, DrawContext, Event, EventKind, Widget, WidgetData, WidgetError};

pub type HorizontalLayoutContainer = LineLayoutContainer<HorDim>;
pub type VerticalLayoutContainer = LineLayoutContainer<VerDim>;

struct LineLayoutContainerData {
    drawn_bounds: LogicalRect,
    placement: WidgetPlacement,
    visible: bool,
    rendered_valid: bool,

    bg_color: [f32; 4],

    children: Vec<Rc<dyn Widget>>,

    /// The idea is that we start the layout by itearting thorugh all the children
    /// and adding up the width (and offset from start or end if any) of fixed-width widgets. This
    /// sum subtracted from the available width gives the amount of space that's left to
    /// distribute between the stretch widgets. Dividing that with the number of stretch widgets
    /// gives the width of each stretch widget.
    /// (This ignores the `min` and `max` fields of `Stretch` but I'll deal with that later.)
    ///
    /// At this point we start calculating the position of each widget starting from those
    /// children that are aligned to the start. After all of those are done, the center-ones follow
    /// and after those, the end-ones. Note that this behaviour means that if the widgets can't fit
    /// inside this container, the end-widgets will fall off at the end first. Then the center-
    /// ones will start to fall off and then the start-ones by continually shrinking the available
    /// space.
    ///
    /// The list of widgets with different alignement are kept cached within the following
    /// containers, maintaining their order from the children container.
    start_children: Vec<Rc<dyn Widget>>,
    center_children: Vec<Rc<dyn Widget>>,
    end_children: Vec<Rc<dyn Widget>>,
}
impl WidgetData for LineLayoutContainerData {
    fn placement(&mut self) -> &mut WidgetPlacement {
        &mut self.placement
    }
    fn drawn_bounds(&mut self) -> &mut LogicalRect {
        &mut self.drawn_bounds
    }
    fn visible(&mut self) -> &mut bool {
        &mut self.visible
    }
}

pub struct LineLayoutContainer<Dim: PickDimension + 'static> {
    data: RefCell<LineLayoutContainerData>,
    phantom: std::marker::PhantomData<Dim>,
}
impl<Dim: PickDimension + 'static> LineLayoutContainer<Dim> {
    pub fn new() -> LineLayoutContainer<Dim> {
        LineLayoutContainer {
            data: RefCell::new(LineLayoutContainerData {
                drawn_bounds: Default::default(),
                placement: Default::default(),
                rendered_valid: false,
                bg_color: [0.0, 0.0, 0.0, 0.0],
                visible: true,
                children: Vec::new(),
                start_children: Vec::new(),
                center_children: Vec::new(),
                end_children: Vec::new(),
            }),
            phantom: Default::default(),
        }
    }

    add_common_widget_functions!(data);

    pub fn set_bg_color(&self, color: [f32; 4]) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.bg_color = color;
        borrowed.rendered_valid = false;
    }

    pub fn add_child(&self, new_child: Rc<dyn Widget>) {
        let mut borrowed = self.data.borrow_mut();
        for child in borrowed.children.iter() {
            if Rc::ptr_eq(&child, &new_child) {
                return;
            }
        }
        borrowed.children.push(new_child);
        borrowed.rendered_valid = false;
    }

    pub fn remove_child(&self, target: Rc<dyn Widget>) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.children.retain(|child| !Rc::ptr_eq(&child, &target));
        borrowed.rendered_valid = false;
    }

    fn layout_aligned_children(
        alignement_group: &[Rc<dyn Widget>],
        stretch_space_per_widget: f32,
        widget_available_space: &mut LogicalRect,
    ) {
        for child in alignement_group.iter() {
            let placement: WidgetPlacement = child.placement();
            let margins = Dim::margin_start(&placement) + Dim::margin_end(&placement);
            match Dim::extent(&placement) {
                Length::Fixed(extent) => {
                    *Dim::rect_size_mut(widget_available_space) = extent + margins;
                }
                Length::Stretch { max, .. } => {
                    if stretch_space_per_widget > 0.0 {
                        let max_space = max + margins;
                        *Dim::rect_size_mut(widget_available_space) = stretch_space_per_widget.min(max_space);
                    }
                }
            }
            child.layout(*widget_available_space);
            *Dim::rect_pos_mut(widget_available_space) += Dim::rect_size(widget_available_space);
        }
    }
}
impl<Dim: PickDimension + 'static> Widget for LineLayoutContainer<Dim> {
    fn is_valid(&self) -> bool {
        self.data.borrow().rendered_valid
    }

    fn before_draw(&self, window: &Window) {
        let borrowed = self.data.borrow();
        if borrowed.visible {
            for child in borrowed.children.iter() {
                child.before_draw(window);
            }
        }
    }

    fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<(), WidgetError> {
        {
            let borrowed = self.data.borrow();
            if !borrowed.visible {
                return Ok(());
            }
            if borrowed.bg_color[3] > 0.0 {
                let viewport_rect = context.logical_rect_to_viewport(&borrowed.drawn_bounds);
                target.clear(
                    Some(&viewport_rect),
                    Some((borrowed.bg_color[0], borrowed.bg_color[1], borrowed.bg_color[2], borrowed.bg_color[3])),
                    false,
                    None,
                    None
                );
            }
            for child in borrowed.children.iter() {
                child.draw(target, context)?;
            }
        }
        self.data.borrow_mut().rendered_valid = true;
        Ok(())
    }

    fn layout(&self, mut available_space: LogicalRect) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.default_layout(available_space);
        if !borrowed.visible {
            return;
        }
        available_space = borrowed.drawn_bounds;

        borrowed.start_children.clear();
        borrowed.center_children.clear();
        borrowed.end_children.clear();

        let mut max_stretch_space = Dim::rect_size(&available_space);
        let mut stretch_widget_count = 0.0;
        let mut center_max_size = 0.0;

        let children_clone = borrowed.children.clone();
        for child in children_clone.iter() {
            if !child.visible() {
                continue;
            }
            let placement: WidgetPlacement = child.placement();
            if placement.ignore_layout {
                child.layout(available_space);
            } else {
                let center;
                match Dim::alignment(&placement) {
                    Alignment::Start => {
                        borrowed.start_children.push(child.clone());
                        center = false;
                    }
                    Alignment::Center => {
                        borrowed.center_children.push(child.clone());
                        center = true;
                    }
                    Alignment::End => {
                        borrowed.end_children.push(child.clone());
                        center = false;
                    }
                }
                let margins = Dim::margin_start(&placement) + Dim::margin_end(&placement);
                match Dim::extent(&placement) {
                    Length::Fixed(extent) => {
                        // Margin only taken away from stertch space
                        max_stretch_space -= extent + margins;
                        if center {
                            center_max_size += extent + margins;
                        }
                    }
                    Length::Stretch { min, max } => {
                        // Widgets have to fit their marings within the available space
                        // therefore the margins of stretch widgets should not be taken
                        // from the available stretch space (i.e. `max_stretch_space`).
                        max_stretch_space -= min;
                        if center {
                            center_max_size += max + margins;
                        }
                        stretch_widget_count += 1.0;
                    }
                }
            }
        }
        let stretch_space_per_widget = max_stretch_space / stretch_widget_count;
        let mut widget_available_space = available_space;
        // Now let's start to place the elements
        Self::layout_aligned_children(
            &borrowed.start_children,
            stretch_space_per_widget,
            &mut widget_available_space,
        );
        let center_pos = Dim::vec(available_space.center());
        let center_start_pos =
            (center_pos - center_max_size * 0.5).max(Dim::rect_pos(&widget_available_space));
        *Dim::rect_pos_mut(&mut widget_available_space) = center_start_pos;
        Self::layout_aligned_children(
            &borrowed.center_children,
            stretch_space_per_widget,
            &mut widget_available_space,
        );
        Self::layout_aligned_children(
            &borrowed.end_children,
            stretch_space_per_widget,
            &mut widget_available_space,
        );
    }

    fn handle_event(&self, event: &Event) {
        let children; 
        {
            let borrowed = self.data.borrow();
            if !borrowed.visible {
                return;
            }
            children = borrowed.children.clone();
        }
        for child in children.iter() {
            child.handle_event(event);
        }
    }

    fn children(&self, children: &mut Vec<Rc<dyn Widget>>) {
        let borrowed = self.data.borrow();
        for child in borrowed.children.iter() {
            children.push(child.clone());
        }
    }

    fn placement(&self) -> WidgetPlacement {
        self.data.borrow().placement
    }

    fn visible(&self) -> bool {
        self.data.borrow().visible
    }
}

