use gelatin::{
    application::*, button::*, line_layout_container::*, misc::*, window::*, picture::*, slider::*
};

use std::cell::Cell;
use std::f32;
use std::rc::Rc;

fn main() {
    let mut application = Application::new();
    // A window
    let window = Window::new(&mut application, WindowDescriptorBuilder::default().build().unwrap());
    let container = Rc::new(HorizontalLayoutContainer::new());
    container.set_margin_top(5.0);
    container.set_margin_bottom(5.0);
    container.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
    container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    let image = Rc::new(Picture::new("examples/resource/cogs.png"));
    let button = Rc::new(Button::new());
    button.set_margin_top(5.0);
    button.set_height(Length::Fixed(24.0));
    button.set_width(Length::Fixed(24.0));
    button.set_horizontal_align(Alignment::Center);
    button.set_icon(Some(image));

    let button2 = Rc::new(Button::new());
    button2.set_margin_top(5.0);
    //button.set_pos(LogicalVector::new(5.0, 5.0));
    //button.set_fixed_size(LogicalVector::new(24.0, 24.0));
    button2.set_height(Length::Fixed(24.0));
    button2.set_width(Length::Fixed(24.0));
    button2.set_horizontal_align(Alignment::Center);

    let slider = Rc::new(Slider::new());
    slider.set_margin_top(5.0);
    slider.set_height(Length::Fixed(24.0));
    slider.set_width(Length::Stretch { min: 0.0, max: 200.0 });
    slider.set_horizontal_align(Alignment::Start);
    slider.set_steps(6, 0);

    container.add_child(button.clone());
    container.add_child(button2.clone());
    container.add_child(slider.clone());

    container.set_margin_left(0.0);
    container.set_margin_right(0.0);
    button.set_margin_left(5.0);
    button.set_margin_right(5.0);
    button2.set_margin_left(5.0);
    button2.set_margin_right(5.0);
    slider.set_margin_left(5.0);
    slider.set_margin_right(5.0);

    let button_clone = button.clone();
    // The closure is Fn (i.e. not mutable) so `pos` has to be wrapped in a `Cell`.
    let pos = Cell::new(5.0);
    button.set_on_click(move || {
        let new_pos = pos.get() + 5.0;
        pos.set(new_pos);

        button_clone.set_margin_left(new_pos);
        button_clone.set_margin_top(new_pos);
    });
    let button_clone2 = button.clone();
    let slider_clone = slider.clone();
    slider.set_on_value_change(move || {
        let margin = (slider_clone.value() + 1) as f32 * 5.0;
        button_clone2.set_margin_right(margin);
    });
    window.set_root(container);
    application.start_event_loop();
}
