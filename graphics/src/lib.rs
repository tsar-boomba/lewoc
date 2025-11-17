#![no_std]
use core::fmt::Debug;

use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_9X15},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
};
use embedded_text::{
    TextBox, alignment::HorizontalAlignment, style::HeightMode, style::TextBoxStyleBuilder,
};

pub fn fill<D: DrawTargetExt<Color = Rgb565>>(target: &mut D)
where
    D::Error: Debug,
{
    target.clear(Rgb565::new(0, 0, 0)).unwrap();
}

pub fn draw_message<D: DrawTargetExt<Color = Rgb565>>(target: &mut D, message: &str)
where
    D::Error: Debug,
{
    let name_text_style = MonoTextStyleBuilder::new()
        .font(&FONT_9X15)
        .text_color(Rgb565::new(255, 0, 0))
        .build();

    // Use height as width of text box since the screen is rotated
    let bounds = Rectangle::new(Point::new(2, 0), Size::new(common::DISPLAY_HEIGHT - 2, 0));

    let textbox_style = TextBoxStyleBuilder::new()
        .height_mode(HeightMode::FitToText)
        .alignment(HorizontalAlignment::Justified)
        .paragraph_spacing(6)
        .build();

    let text_box = TextBox::with_textbox_style(message, bounds, name_text_style, textbox_style);

    text_box.draw(target).unwrap();
}
