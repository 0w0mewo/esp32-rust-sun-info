use crate::ui::{PRIMITIVE_STYLE_DEFAULT, polar};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii},
    pixelcolor,
    prelude::*,
    primitives::{Circle, Line, StyledDrawable},
    text::{Baseline, Text},
};

/// draw line with polar coordinate
pub(crate) struct PolarLine<'a> {
    center: Point,
    angle: f64,
    radius: f64,
    label: Option<&'a str>,
}
impl<'a> PolarLine<'a> {
    pub fn new(center: Point, angle: f64, radius: f64) -> Self {
        Self {
            center,
            angle,
            radius,
            label: None,
        }
    }

    pub fn with_label(center: Point, angle: f64, radius: f64, label: &'a str) -> Self {
        let mut p = Self::new(center, angle, radius);
        p.label.replace(label);
        p
    }
}

impl Drawable for PolarLine<'_> {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let p = polar(self.center, self.angle, self.radius);

        // line
        Line::new(self.center, p).draw_styled(&PRIMITIVE_STYLE_DEFAULT, target)?;

        // label
        if let Some(label) = self.label {
            Text::with_baseline(
                label,
                p,
                MonoTextStyle::new(&ascii::FONT_5X7, pixelcolor::BinaryColor::On),
                Baseline::Middle,
            )
            .draw(target)?;
        } else {
            Circle::with_center(p, 2).draw_styled(&PRIMITIVE_STYLE_DEFAULT, target)?;
        }

        Ok(())
    }
}

/// the status text shared by all views
pub(crate) struct CommonStatusTexts<'a> {
    s: &'a str,
    position: Point,
}

impl<'a> CommonStatusTexts<'a> {
    pub fn new(position: Point, s: &'a str) -> Self {
        Self { s, position }
    }
}

impl Drawable for CommonStatusTexts<'_> {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        // shrink to a smaller font size if the lines exceeded the screen
        let font = if self.s.lines().count() > 8 {
            &ascii::FONT_5X7
        } else {
            &ascii::FONT_5X8
        };

        Text::with_baseline(
            self.s,
            self.position,
            MonoTextStyle::new(font, pixelcolor::BinaryColor::On),
            Baseline::Top,
        )
        .draw(target)?;

        Ok(())
    }
}

/// compass primitive
pub(crate) struct Compass {
    pub center: Point,
    pub diameter: u32,
}

impl Compass {
    pub fn new(center: Point, diameter: u32) -> Self {
        Self { center, diameter }
    }
}

impl Drawable for Compass {
    type Color = pixelcolor::BinaryColor;

    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        let face = Circle::with_center(self.center, self.diameter);
        face.draw_styled(&PRIMITIVE_STYLE_DEFAULT, target)?;

        for &(angle, angle_txt) in &[(0.0, "N"), (90.0, "E"), (180.0, "S"), (270.0, "W")] {
            let pos = polar(self.center, angle, 0.5 * self.diameter as f64 - 4.0);
            Text::with_baseline(
                angle_txt,
                pos,
                MonoTextStyle::new(&ascii::FONT_4X6, pixelcolor::BinaryColor::On),
                Baseline::Middle,
            )
            .draw(target)?;
        }

        Ok(())
    }
}
