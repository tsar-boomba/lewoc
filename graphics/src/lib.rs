#![no_std]

use core::fmt::Debug;

use common::Station;
use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_6X9},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};
use embedded_text::{
    TextBox, alignment::HorizontalAlignment, style::HeightMode, style::TextBoxStyleBuilder,
};

pub fn draw_station_name<D: DrawTargetExt<Color = Rgb565>>(target: &mut D, station: Station)
where
    D::Error: Debug,
{
    let name_text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X9)
        .text_color(Rgb565::new(255, 0, 0))
        .build();

    let bounds = Rectangle::new(Point::new(2, 0), Size::new(124, 0));

    let textbox_style = TextBoxStyleBuilder::new()
        .height_mode(HeightMode::FitToText)
        .alignment(HorizontalAlignment::Justified)
        .paragraph_spacing(6)
        .build();

    let text_box = TextBox::with_textbox_style(
        "HELLO NIX YOU ARE A GREAT PERSON AND GREAT FRIEND AND STUFF HII!",
        bounds,
        name_text_style,
        textbox_style,
    );

    let name = Text::with_baseline(
        "Hey Ibomb!",
        Point::new(0, 0),
        name_text_style,
        Baseline::Top,
    );

    text_box.draw(target).unwrap();
}
