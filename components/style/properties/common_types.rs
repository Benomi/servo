/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![allow(non_camel_case_types)]

use url::{Url, UrlParser};

pub use servo_util::geometry::Au;

pub type CSSFloat = f64;

pub mod specified {
    use std::ascii::AsciiExt;
    use std::f64::consts::PI;
    use std::fmt;
    use std::fmt::{Formatter, FormatError, Show};
    use url::Url;
    use cssparser;
    use cssparser::ast;
    use cssparser::ast::*;
    use parsing_utils::{mod, BufferedIter, ParserIter};
    use super::{Au, CSSFloat};
    #[deriving(Clone, PartialEq)]
    pub struct CSSColor {
        pub parsed: cssparser::Color,
        pub authored: Option<String>,
    }
    impl CSSColor {
        pub fn parse(component_value: &ComponentValue) -> Result<CSSColor, ()> {
            let parsed = cssparser::Color::parse(component_value);
            parsed.map(|parsed| {
                let authored = match component_value {
                    &Ident(ref s) => Some(s.clone()),
                    _ => None,
                };
                CSSColor {
                    parsed: parsed,
                    authored: authored,
                }
            })
        }
    }
    impl fmt::Show for CSSColor {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self.authored {
                Some(ref s) => write!(f, "{}", s),
                None => write!(f, "{}", self.parsed),
            }
        }
    }

    #[deriving(Clone)]
    pub struct CSSRGBA {
        pub parsed: cssparser::RGBA,
        pub authored: Option<String>,
    }
    impl fmt::Show for CSSRGBA {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self.authored {
                Some(ref s) => write!(f, "{}", s),
                None => write!(f, "{}", self.parsed),
            }
        }
    }

    #[deriving(Clone, PartialEq)]
    pub struct CSSImage(pub Option<Image>);
    impl fmt::Show for CSSImage {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let &CSSImage(ref url) = self;
            match url {
                &Some(ref image) => write!(f, "{}", image),
                &None => write!(f, "none"),
            }
        }
    }

    #[deriving(Clone, PartialEq)]
    pub enum Length {
        Au(Au),  // application units
        Em(CSSFloat),
        Ex(CSSFloat),
        Rem(CSSFloat),

        /// HTML5 "character width", as defined in HTML5 § 14.5.4.
        ///
        /// This cannot be specified by the user directly and is only generated by
        /// `Stylist::synthesize_rules_for_legacy_attributes()`.
        ServoCharacterWidth(i32),

        // XXX uncomment when supported:
//        Ch(CSSFloat),
//        Vw(CSSFloat),
//        Vh(CSSFloat),
//        Vmin(CSSFloat),
//        Vmax(CSSFloat),
    }
    impl fmt::Show for Length {
        fn fmt(&self, f: &mut Formatter) -> fmt::Result {
            match self {
                &Length::Au(length) => write!(f, "{}", length),
                &Length::Em(length) => write!(f, "{}em", length),
                &Length::Ex(length) => write!(f, "{}ex", length),
                &Length::Rem(length) => write!(f, "{}rem", length),
                &Length::ServoCharacterWidth(_) => panic!("internal CSS values should never be serialized"),
            }
        }
    }
    const AU_PER_PX: CSSFloat = 60.;
    const AU_PER_IN: CSSFloat = AU_PER_PX * 96.;
    const AU_PER_CM: CSSFloat = AU_PER_IN / 2.54;
    const AU_PER_MM: CSSFloat = AU_PER_IN / 25.4;
    const AU_PER_PT: CSSFloat = AU_PER_IN / 72.;
    const AU_PER_PC: CSSFloat = AU_PER_PT * 12.;
    impl Length {
        #[inline]
        fn parse_internal(input: &ComponentValue, negative_ok: bool) -> Result<Length, ()> {
            match input {
                &Dimension(ref value, ref unit) if negative_ok || value.value >= 0.
                => Length::parse_dimension(value.value, unit.as_slice()),
                &Number(ref value) if value.value == 0. =>  Ok(Length::Au(Au(0))),
                _ => Err(())
            }
        }
        #[allow(dead_code)]
        pub fn parse(input: &ComponentValue) -> Result<Length, ()> {
            Length::parse_internal(input, /* negative_ok = */ true)
        }
        pub fn parse_non_negative(input: &ComponentValue) -> Result<Length, ()> {
            Length::parse_internal(input, /* negative_ok = */ false)
        }
        pub fn parse_dimension(value: CSSFloat, unit: &str) -> Result<Length, ()> {
            match unit.to_ascii_lower().as_slice() {
                "px" => Ok(Length::from_px(value)),
                "in" => Ok(Length::Au(Au((value * AU_PER_IN) as i32))),
                "cm" => Ok(Length::Au(Au((value * AU_PER_CM) as i32))),
                "mm" => Ok(Length::Au(Au((value * AU_PER_MM) as i32))),
                "pt" => Ok(Length::Au(Au((value * AU_PER_PT) as i32))),
                "pc" => Ok(Length::Au(Au((value * AU_PER_PC) as i32))),
                "em" => Ok(Length::Em(value)),
                "ex" => Ok(Length::Ex(value)),
                "rem" => Ok(Length::Rem(value)),
                _ => Err(())
            }
        }
        #[inline]
        pub fn from_px(px_value: CSSFloat) -> Length {
            Length::Au(Au((px_value * AU_PER_PX) as i32))
        }
    }

    #[deriving(Clone, PartialEq)]
    pub enum LengthOrPercentage {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
    }
    impl fmt::Show for LengthOrPercentage {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentage::Length(length) => write!(f, "{}", length),
                &LengthOrPercentage::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
            }
        }
    }
    impl LengthOrPercentage {
        fn parse_internal(input: &ComponentValue, negative_ok: bool)
                              -> Result<LengthOrPercentage, ()> {
            match input {
                &Dimension(ref value, ref unit) if negative_ok || value.value >= 0. =>
                    Length::parse_dimension(value.value, unit.as_slice())
                        .map(LengthOrPercentage::Length),
                &ast::Percentage(ref value) if negative_ok || value.value >= 0. =>
                    Ok(LengthOrPercentage::Percentage(value.value / 100.)),
                &Number(ref value) if value.value == 0. =>
                    Ok(LengthOrPercentage::Length(Length::Au(Au(0)))),
                _ => Err(())
            }
        }
        #[allow(dead_code)]
        #[inline]
        pub fn parse(input: &ComponentValue) -> Result<LengthOrPercentage, ()> {
            LengthOrPercentage::parse_internal(input, /* negative_ok = */ true)
        }
        #[inline]
        pub fn parse_non_negative(input: &ComponentValue) -> Result<LengthOrPercentage, ()> {
            LengthOrPercentage::parse_internal(input, /* negative_ok = */ false)
        }
    }

    #[deriving(Clone)]
    pub enum LengthOrPercentageOrAuto {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
        Auto,
    }
    impl fmt::Show for LengthOrPercentageOrAuto {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentageOrAuto::Length(length) => write!(f, "{}", length),
                &LengthOrPercentageOrAuto::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
                &LengthOrPercentageOrAuto::Auto => write!(f, "auto"),
            }
        }
    }
    impl LengthOrPercentageOrAuto {
        fn parse_internal(input: &ComponentValue, negative_ok: bool)
                     -> Result<LengthOrPercentageOrAuto, ()> {
            match input {
                &Dimension(ref value, ref unit) if negative_ok || value.value >= 0. =>
                    Length::parse_dimension(value.value, unit.as_slice()).map(LengthOrPercentageOrAuto::Length),
                &ast::Percentage(ref value) if negative_ok || value.value >= 0. =>
                    Ok(LengthOrPercentageOrAuto::Percentage(value.value / 100.)),
                &Number(ref value) if value.value == 0. =>
                    Ok(LengthOrPercentageOrAuto::Length(Length::Au(Au(0)))),
                &Ident(ref value) if value.as_slice().eq_ignore_ascii_case("auto") =>
                    Ok(LengthOrPercentageOrAuto::Auto),
                _ => Err(())
            }
        }
        #[inline]
        pub fn parse(input: &ComponentValue) -> Result<LengthOrPercentageOrAuto, ()> {
            LengthOrPercentageOrAuto::parse_internal(input, /* negative_ok = */ true)
        }
        #[inline]
        pub fn parse_non_negative(input: &ComponentValue) -> Result<LengthOrPercentageOrAuto, ()> {
            LengthOrPercentageOrAuto::parse_internal(input, /* negative_ok = */ false)
        }
    }

    #[deriving(Clone)]
    pub enum LengthOrPercentageOrNone {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
        None,
    }
    impl fmt::Show for LengthOrPercentageOrNone {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentageOrNone::Length(length) => write!(f, "{}", length),
                &LengthOrPercentageOrNone::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
                &LengthOrPercentageOrNone::None => write!(f, "none"),
            }
        }
    }
    impl LengthOrPercentageOrNone {
        fn parse_internal(input: &ComponentValue, negative_ok: bool)
                     -> Result<LengthOrPercentageOrNone, ()> {
            match input {
                &Dimension(ref value, ref unit) if negative_ok || value.value >= 0.
                => Length::parse_dimension(value.value, unit.as_slice()).map(LengthOrPercentageOrNone::Length),
                &ast::Percentage(ref value) if negative_ok || value.value >= 0.
                => Ok(LengthOrPercentageOrNone::Percentage(value.value / 100.)),
                &Number(ref value) if value.value == 0. => Ok(LengthOrPercentageOrNone::Length(Length::Au(Au(0)))),
                &Ident(ref value) if value.as_slice().eq_ignore_ascii_case("none") => Ok(LengthOrPercentageOrNone::None),
                _ => Err(())
            }
        }
        #[allow(dead_code)]
        #[inline]
        pub fn parse(input: &ComponentValue) -> Result<LengthOrPercentageOrNone, ()> {
            LengthOrPercentageOrNone::parse_internal(input, /* negative_ok = */ true)
        }
        #[inline]
        pub fn parse_non_negative(input: &ComponentValue) -> Result<LengthOrPercentageOrNone, ()> {
            LengthOrPercentageOrNone::parse_internal(input, /* negative_ok = */ false)
        }
    }

    // http://dev.w3.org/csswg/css2/colors.html#propdef-background-position
    #[deriving(Clone)]
    pub enum PositionComponent {
        Length(Length),
        Percentage(CSSFloat),  // [0 .. 100%] maps to [0.0 .. 1.0]
        Center,
        Left,
        Right,
        Top,
        Bottom,
    }
    impl PositionComponent {
        pub fn parse(input: &ComponentValue) -> Result<PositionComponent, ()> {
            match input {
                &Dimension(ref value, ref unit) =>
                    Length::parse_dimension(value.value, unit.as_slice()).map(PositionComponent::Length),
                &ast::Percentage(ref value) => Ok(PositionComponent::Percentage(value.value / 100.)),
                &Number(ref value) if value.value == 0. => Ok(PositionComponent::Length(Length::Au(Au(0)))),
                &Ident(ref value) => {
                    if value.as_slice().eq_ignore_ascii_case("center") { Ok(PositionComponent::Center) }
                    else if value.as_slice().eq_ignore_ascii_case("left") { Ok(PositionComponent::Left) }
                    else if value.as_slice().eq_ignore_ascii_case("right") { Ok(PositionComponent::Right) }
                    else if value.as_slice().eq_ignore_ascii_case("top") { Ok(PositionComponent::Top) }
                    else if value.as_slice().eq_ignore_ascii_case("bottom") { Ok(PositionComponent::Bottom) }
                    else { Err(()) }
                }
                _ => Err(())
            }
        }
        #[inline]
        pub fn to_length_or_percentage(self) -> LengthOrPercentage {
            match self {
                PositionComponent::Length(x) => LengthOrPercentage::Length(x),
                PositionComponent::Percentage(x) => LengthOrPercentage::Percentage(x),
                PositionComponent::Center => LengthOrPercentage::Percentage(0.5),
                PositionComponent::Left | PositionComponent::Top => LengthOrPercentage::Percentage(0.0),
                PositionComponent::Right | PositionComponent::Bottom => LengthOrPercentage::Percentage(1.0),
            }
        }
    }

    #[deriving(Clone, PartialEq, PartialOrd)]
    pub struct Angle(pub CSSFloat);

    impl Show for Angle {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
            let Angle(value) = *self;
            write!(f, "{}", value)
        }
    }

    impl Angle {
        pub fn radians(self) -> f64 {
            let Angle(radians) = self;
            radians
        }
    }

    static DEG_TO_RAD: CSSFloat = PI / 180.0;
    static GRAD_TO_RAD: CSSFloat = PI / 200.0;

    impl Angle {
        /// Parses an angle according to CSS-VALUES § 6.1.
        fn parse_dimension(value: CSSFloat, unit: &str) -> Result<Angle,()> {
            if unit.eq_ignore_ascii_case("deg") {
                Ok(Angle(value * DEG_TO_RAD))
            } else if unit.eq_ignore_ascii_case("grad") {
                Ok(Angle(value * GRAD_TO_RAD))
            } else if unit.eq_ignore_ascii_case("rad") {
                Ok(Angle(value))
            } else if unit.eq_ignore_ascii_case("turn") {
                Ok(Angle(value * 2.0 * PI))
            } else {
                Err(())
            }
        }
    }

    /// Specified values for an image according to CSS-IMAGES.
    #[deriving(Clone, PartialEq)]
    pub enum Image {
        Url(Url),
        LinearGradient(LinearGradient),
    }

    impl Show for Image {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
            match self {
                &Image::Url(ref url) => write!(f, "url(\"{}\")", url),
                &Image::LinearGradient(ref grad) => write!(f, "linear-gradient({})", grad),
            }
        }
    }

    impl Image {
        pub fn from_component_value(component_value: &ComponentValue, base_url: &Url)
                                    -> Result<Image,()> {
            match component_value {
                &ast::URL(ref url) => {
                    let image_url = super::parse_url(url.as_slice(), base_url);
                    Ok(Image::Url(image_url))
                },
                &ast::Function(ref name, ref args) => {
                    if name.as_slice().eq_ignore_ascii_case("linear-gradient") {
                        Ok(Image::LinearGradient(try!(
                                    super::specified::LinearGradient::parse_function(
                                    args.as_slice()))))
                    } else {
                        Err(())
                    }
                }
                _ => Err(()),
            }
        }

        pub fn to_computed_value(self, context: &super::computed::Context)
                                 -> super::computed::Image {
            match self {
                Image::Url(url) => super::computed::Image::Url(url),
                Image::LinearGradient(linear_gradient) => {
                    super::computed::Image::LinearGradient(
                        super::computed::LinearGradient::compute(linear_gradient, context))
                }
            }
        }
    }

    /// Specified values for a CSS linear gradient.
    #[deriving(Clone, PartialEq)]
    pub struct LinearGradient {
        /// The angle or corner of the gradient.
        pub angle_or_corner: AngleOrCorner,

        /// The color stops.
        pub stops: Vec<ColorStop>,
    }

    impl Show for LinearGradient {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
            let _ = write!(f, "{}", self.angle_or_corner);
            for stop in self.stops.iter() {
                let _ = write!(f, ", {}", stop);
            }
            Ok(())
        }
    }

    /// Specified values for an angle or a corner in a linear gradient.
    #[deriving(Clone, PartialEq)]
    pub enum AngleOrCorner {
        Angle(Angle),
        Corner(HorizontalDirection, VerticalDirection),
    }

    impl Show for AngleOrCorner {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
            match self {
                &AngleOrCorner::Angle(angle) => write!(f, "{}", angle),
                &AngleOrCorner::Corner(horiz, vert) => write!(f, "to {} {}", horiz, vert),
            }
        }
    }

    /// Specified values for one color stop in a linear gradient.
    #[deriving(Clone, PartialEq)]
    pub struct ColorStop {
        /// The color of this stop.
        pub color: CSSColor,

        /// The position of this stop. If not specified, this stop is placed halfway between the
        /// point that precedes it and the point that follows it.
        pub position: Option<LengthOrPercentage>,
    }

    impl Show for ColorStop {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
            let _ = write!(f, "{}", self.color);
            self.position.map(|pos| {
                let _ = write!(f, " {}", pos);
            });
            Ok(())
        }
    }

    #[deriving(Clone, PartialEq)]
    pub enum HorizontalDirection {
        Left,
        Right,
    }

    impl Show for HorizontalDirection {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
            match self {
                &HorizontalDirection::Left => write!(f, "left"),
                &HorizontalDirection::Right => write!(f, "right"),
            }
        }
    }

    #[deriving(Clone, PartialEq)]
    pub enum VerticalDirection {
        Top,
        Bottom,
    }

    impl Show for VerticalDirection {
        fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
            match self {
                &VerticalDirection::Top => write!(f, "top"),
                &VerticalDirection::Bottom => write!(f, "bottom"),
            }
        }
    }

    fn parse_color_stop(source: ParserIter) -> Result<ColorStop,()> {
        let color = match source.next() {
            Some(color) => try!(CSSColor::parse(color)),
            None => return Err(()),
        };

        let position = match source.next() {
            None => None,
            Some(value) => {
                match *value {
                    Comma => {
                        source.push_back(value);
                        None
                    }
                    ref position => Some(try!(LengthOrPercentage::parse(position))),
                }
            }
        };

        Ok(ColorStop {
            color: color,
            position: position,
        })
    }

    impl LinearGradient {
        /// Parses a linear gradient from the given arguments.
        pub fn parse_function(args: &[ComponentValue]) -> Result<LinearGradient,()> {
            let mut source = BufferedIter::new(args.skip_whitespace());

            // Parse the angle.
            let (angle_or_corner, need_to_parse_comma) = match source.next() {
                None => return Err(()),
                Some(token) => {
                    match *token {
                        Dimension(ref value, ref unit) => {
                            match Angle::parse_dimension(value.value, unit.as_slice()) {
                                Ok(angle) => {
                                    (AngleOrCorner::Angle(angle), true)
                                }
                                Err(()) => {
                                    source.push_back(token);
                                    (AngleOrCorner::Angle(Angle(PI)), false)
                                }
                            }
                        }
                        Ident(ref ident) if ident.as_slice().eq_ignore_ascii_case("to") => {
                            let (mut horizontal, mut vertical) = (None, None);
                            loop {
                                match source.next() {
                                    None => break,
                                    Some(token) => {
                                        match *token {
                                            Ident(ref ident) => {
                                                let ident = ident.as_slice();
                                                if ident.eq_ignore_ascii_case("top") &&
                                                        vertical.is_none() {
                                                    vertical = Some(VerticalDirection::Top)
                                                } else if ident.eq_ignore_ascii_case("bottom") &&
                                                        vertical.is_none() {
                                                    vertical = Some(VerticalDirection::Bottom)
                                                } else if ident.eq_ignore_ascii_case("left") &&
                                                        horizontal.is_none() {
                                                    horizontal = Some(HorizontalDirection::Left)
                                                } else if ident.eq_ignore_ascii_case("right") &&
                                                        horizontal.is_none() {
                                                    horizontal = Some(HorizontalDirection::Right)
                                                } else {
                                                    return Err(())
                                                }
                                            }
                                            Comma => {
                                                source.push_back(token);
                                                break
                                            }
                                            _ => return Err(()),
                                        }
                                    }
                                }
                            }

                            (match (horizontal, vertical) {
                                (None, Some(VerticalDirection::Top)) => {
                                    AngleOrCorner::Angle(Angle(0.0))
                                },
                                (Some(HorizontalDirection::Right), None) => {
                                    AngleOrCorner::Angle(Angle(PI * 0.5))
                                },
                                (None, Some(VerticalDirection::Bottom)) => {
                                    AngleOrCorner::Angle(Angle(PI))
                                },
                                (Some(HorizontalDirection::Left), None) => {
                                    AngleOrCorner::Angle(Angle(PI * 1.5))
                                },
                                (Some(horizontal), Some(vertical)) => {
                                    AngleOrCorner::Corner(horizontal, vertical)
                                }
                                (None, None) => return Err(()),
                            }, true)
                        }
                        _ => {
                            source.push_back(token);
                            (AngleOrCorner::Angle(Angle(PI)), false)
                        }
                    }
                }
            };

            // Parse the color stops.
            let stops = if need_to_parse_comma {
                match source.next() {
                    Some(&Comma) => {
                        try!(parsing_utils::parse_comma_separated(&mut source, parse_color_stop))
                    }
                    None => Vec::new(),
                    Some(_) => return Err(()),
                }
            } else {
                try!(parsing_utils::parse_comma_separated(&mut source, parse_color_stop))
            };

            if stops.len() < 2 {
                return Err(())
            }

            Ok(LinearGradient {
                angle_or_corner: angle_or_corner,
                stops: stops,
            })
        }
    }
}

pub mod computed {
    pub use super::specified::{Angle, AngleOrCorner, HorizontalDirection};
    pub use super::specified::{VerticalDirection};
    pub use cssparser::Color as CSSColor;
    use super::*;
    use super::super::longhands;
    use std::fmt;
    use url::Url;

    pub struct Context {
        pub inherited_font_weight: longhands::font_weight::computed_value::T,
        pub inherited_font_size: longhands::font_size::computed_value::T,
        pub inherited_text_decorations_in_effect: longhands::_servo_text_decorations_in_effect::T,
        pub inherited_height: longhands::height::T,
        pub color: longhands::color::computed_value::T,
        pub text_decoration: longhands::text_decoration::computed_value::T,
        pub font_size: longhands::font_size::computed_value::T,
        pub root_font_size: longhands::font_size::computed_value::T,
        pub display: longhands::display::computed_value::T,
        pub positioned: bool,
        pub floated: bool,
        pub border_top_present: bool,
        pub border_right_present: bool,
        pub border_bottom_present: bool,
        pub border_left_present: bool,
        pub is_root_element: bool,
        // TODO, as needed: viewport size, etc.
    }

    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_CSSColor(value: specified::CSSColor, _context: &computed::Context) -> CSSColor {
        value.parsed
    }

    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_Au(value: specified::Length, context: &Context) -> Au {
        compute_Au_with_font_size(value, context.font_size, context.root_font_size)
    }

    /// A special version of `compute_Au` used for `font-size`.
    #[allow(non_snake_case)]
    #[inline]
    pub fn compute_Au_with_font_size(value: specified::Length, reference_font_size: Au, root_font_size: Au) -> Au {
        match value {
            specified::Length::Au(value) => value,
            specified::Length::Em(value) => reference_font_size.scale_by(value),
            specified::Length::Ex(value) => {
                let x_height = 0.5;  // TODO: find that from the font
                reference_font_size.scale_by(value * x_height)
            },
            specified::Length::Rem(value) => root_font_size.scale_by(value),
            specified::Length::ServoCharacterWidth(value) => {
                // This applies the *converting a character width to pixels* algorithm as specified
                // in HTML5 § 14.5.4.
                //
                // TODO(pcwalton): Find these from the font.
                let average_advance = reference_font_size.scale_by(0.5);
                let max_advance = reference_font_size;
                average_advance.scale_by(value as CSSFloat - 1.0) + max_advance
            }
        }
    }

    #[deriving(PartialEq, Clone)]
    pub enum LengthOrPercentage {
        Length(Au),
        Percentage(CSSFloat),
    }
    impl fmt::Show for LengthOrPercentage {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentage::Length(length) => write!(f, "{}", length),
                &LengthOrPercentage::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
            }
        }
    }

    #[allow(non_snake_case)]
    pub fn compute_LengthOrPercentage(value: specified::LengthOrPercentage, context: &Context)
                                   -> LengthOrPercentage {
        match value {
            specified::LengthOrPercentage::Length(value) =>
                LengthOrPercentage::Length(compute_Au(value, context)),
            specified::LengthOrPercentage::Percentage(value) =>
                LengthOrPercentage::Percentage(value),
        }
    }

    #[deriving(PartialEq, Clone)]
    pub enum LengthOrPercentageOrAuto {
        Length(Au),
        Percentage(CSSFloat),
        Auto,
    }
    impl fmt::Show for LengthOrPercentageOrAuto {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentageOrAuto::Length(length) => write!(f, "{}", length),
                &LengthOrPercentageOrAuto::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
                &LengthOrPercentageOrAuto::Auto => write!(f, "auto"),
            }
        }
    }
    #[allow(non_snake_case)]
    pub fn compute_LengthOrPercentageOrAuto(value: specified::LengthOrPercentageOrAuto,
                                            context: &Context) -> LengthOrPercentageOrAuto {
        match value {
            specified::LengthOrPercentageOrAuto::Length(value) =>
                LengthOrPercentageOrAuto::Length(compute_Au(value, context)),
            specified::LengthOrPercentageOrAuto::Percentage(value) =>
                LengthOrPercentageOrAuto::Percentage(value),
            specified::LengthOrPercentageOrAuto::Auto =>
                LengthOrPercentageOrAuto::Auto,
        }
    }

    #[deriving(PartialEq, Clone)]
    pub enum LengthOrPercentageOrNone {
        Length(Au),
        Percentage(CSSFloat),
        None,
    }
    impl fmt::Show for LengthOrPercentageOrNone {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &LengthOrPercentageOrNone::Length(length) => write!(f, "{}", length),
                &LengthOrPercentageOrNone::Percentage(percentage) => write!(f, "{}%", percentage * 100.),
                &LengthOrPercentageOrNone::None => write!(f, "none"),
            }
        }
    }
    #[allow(non_snake_case)]
    pub fn compute_LengthOrPercentageOrNone(value: specified::LengthOrPercentageOrNone,
                                            context: &Context) -> LengthOrPercentageOrNone {
        match value {
            specified::LengthOrPercentageOrNone::Length(value) =>
                LengthOrPercentageOrNone::Length(compute_Au(value, context)),
            specified::LengthOrPercentageOrNone::Percentage(value) =>
                LengthOrPercentageOrNone::Percentage(value),
            specified::LengthOrPercentageOrNone::None =>
                LengthOrPercentageOrNone::None,
        }
    }

    /// Computed values for an image according to CSS-IMAGES.
    #[deriving(Clone, PartialEq)]
    pub enum Image {
        Url(Url),
        LinearGradient(LinearGradient),
    }

    impl fmt::Show for Image {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                &Image::Url(ref url) => write!(f, "url(\"{}\")", url),
                &Image::LinearGradient(ref grad) => write!(f, "linear-gradient({})", grad),
            }
        }
    }

    /// Computed values for a CSS linear gradient.
    #[deriving(Clone, PartialEq)]
    pub struct LinearGradient {
        /// The angle or corner of the gradient.
        pub angle_or_corner: AngleOrCorner,

        /// The color stops.
        pub stops: Vec<ColorStop>,
    }

    impl fmt::Show for LinearGradient {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let _ = write!(f, "{}", self.angle_or_corner);
            for stop in self.stops.iter() {
                let _ = write!(f, ", {}", stop);
            }
            Ok(())
        }
    }

    /// Computed values for one color stop in a linear gradient.
    #[deriving(Clone, PartialEq)]
    pub struct ColorStop {
        /// The color of this stop.
        pub color: CSSColor,

        /// The position of this stop. If not specified, this stop is placed halfway between the
        /// point that precedes it and the point that follows it per CSS-IMAGES § 3.4.
        pub position: Option<LengthOrPercentage>,
    }

    impl fmt::Show for ColorStop {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let _ = write!(f, "{}", self.color);
            self.position.map(|pos| {
                let _ = write!(f, " {}", pos);
            });
            Ok(())
        }
    }

    impl LinearGradient {
        pub fn compute(value: specified::LinearGradient, context: &Context) -> LinearGradient {
            let specified::LinearGradient {
                angle_or_corner,
                stops
            } = value;
            LinearGradient {
                angle_or_corner: angle_or_corner,
                stops: stops.into_iter().map(|stop| {
                    ColorStop {
                        color: stop.color.parsed,
                        position: match stop.position {
                            None => None,
                            Some(value) => Some(compute_LengthOrPercentage(value, context)),
                        },
                    }
                }).collect()
            }
        }
    }
}

pub fn parse_url(input: &str, base_url: &Url) -> Url {
    UrlParser::new().base_url(base_url).parse(input)
        .unwrap_or_else(|_| Url::parse("about:invalid").unwrap())
}
