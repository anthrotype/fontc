use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};
use thiserror::Error;

use fontdrasil::{
    coords::{NormalizedCoord, NormalizedLocation},
    types::Tag,
    variations::VariationModel,
};
use kurbo::{Affine, Vec2};
use smol_str::SmolStr;

use crate::{Component, Glyph, Layer, Node, Shape, font::AxisPole};

/// Things that can go wrong when instantiating a smart component
#[derive(Debug, Error)]
pub enum BadSmartComponent {
    #[error("unknown component axis '{0}'")]
    UnknownAxis(SmolStr),
    #[error("strange mapping for axis '{axis}': base {base:?} child {child:?}")]
    StrangeAxisMapping {
        base: Option<AxisPole>,
        child: AxisPole,
        axis: SmolStr,
    },
}

/// Instantiate an instance of a smart component.
///
/// A smart component is a glyph that defines its own little variation space,
/// such that specific instances of the glyph can be included as components of
/// other glyphs.
///
/// See <https://glyphsapp.com/learn/smart-components>.
///
/// This code is based on <https://github.com/googlefonts/glyphsLib/blob/52c982399b/Lib/glyphsLib/builder/smart_components.py#L96>
pub(crate) fn instantiate_for_layer(
    layer_master_id: &str,
    component: &Component,
    ref_glyph: &Glyph,
) -> Result<Vec<Shape>, BadSmartComponent> {
    assert!(!component.smart_component_values.is_empty());
    assert!(!ref_glyph.smart_component_axes.is_empty());
    let (axis_order, name_to_tag_map) = axes_for_glyph(ref_glyph);

    // these are the layers of the glyph that have the same associated master
    // as the layer that we are instantiating for.
    let relevant_layers = ref_glyph
        .layers
        .iter()
        .filter(|layer| {
            !layer.smart_component_positions.is_empty() && layer.master_id() == layer_master_id
        })
        .collect::<Vec<_>>();

    if relevant_layers.len() == 1 {
        log::debug!("smart component {} only has one layer?", component.name);
        let mut shapes = relevant_layers[0].shapes.clone();
        shapes
            .iter_mut()
            .for_each(|shape| shape.apply_affine(component.transform));
        return Ok(shapes);
    }

    let locations = relevant_layers
        .iter()
        .map(|layer| normalized_location(layer, relevant_layers[0], &name_to_tag_map))
        .collect::<Result<_, _>>()?;

    let model = VariationModel::new(locations, axis_order.clone());

    let axis_tuples = ref_glyph
        .smart_component_axes
        .iter()
        .map(|(name, range)| {
            let default_value = if relevant_layers[0]
                .smart_component_positions
                .get(name)
                .copied()
                == Some(AxisPole::Min)
            {
                *range.start()
            } else {
                *range.end()
            };
            (name, (*range.start(), default_value, *range.end()))
        })
        .collect::<HashMap<_, _>>();

    let location: NormalizedLocation = component
        .smart_component_values
        .iter()
        .map(|(name, value)| {
            (
                *name_to_tag_map.get(name).unwrap(),
                normalize_value_with_extrapolation(*value, *axis_tuples.get(name).unwrap()),
            )
        })
        .collect();

    log::debug!(
        "instantiating component '{}' at {location:?}",
        component.name
    );

    let point_seqs = relevant_layers
        .iter()
        .map(|layer| {
            let loc = normalized_location(layer, relevant_layers[0], &name_to_tag_map)?;
            let points = layer
                .shapes
                .iter()
                .filter_map(Shape::as_path)
                .flat_map(|path| path.nodes.iter().map(|node| node.pt))
                .collect::<Vec<_>>();
            Ok((loc, points))
        })
        .collect::<Result<HashMap<_, _>, BadSmartComponent>>()?;
    let deltas = model.deltas(&point_seqs).unwrap();
    let points = VariationModel::interpolate_from_deltas(&location, &deltas);
    let mut shapes = shapes_with_new_points(relevant_layers[0], &points);
    shapes
        .iter_mut()
        .for_each(|shape| shape.apply_affine(component.transform));

    Ok(shapes)
}

// component parts just have names, not tags, but VariationModel needs tags;
// we give them tags like `ax01`, `ax02`.. etc
fn axes_for_glyph(glyph: &Glyph) -> (Vec<Tag>, BTreeMap<SmolStr, Tag>) {
    let name_to_tag_map: BTreeMap<_, _> = glyph
        .smart_component_axes
        .keys()
        .enumerate()
        .map(|(i, name)| {
            (
                name.clone(),
                Tag::from_str(&format!("ax{i:02}")).expect("only 99 axes supported!"),
            )
        })
        .collect();
    let axis_order = name_to_tag_map.values().copied().collect();

    (axis_order, name_to_tag_map)
}

fn shapes_with_new_points(layer: &Layer, points: &[Vec2]) -> Vec<Shape> {
    let mut points = points;
    layer
        .shapes
        .iter()
        .map(|shape| match shape {
            Shape::Path(path) => {
                let mut path = path.to_owned();
                for (node, newpt) in path.nodes.iter_mut().zip(points) {
                    node.pt = newpt.to_point();
                }
                points = &points[path.nodes.len()..];
                Shape::Path(path)
            }
            // we just skip components, which matches fonttools. Should we error instead?
            Shape::Component(_) => shape.clone(),
        })
        .collect()
}

//https://github.com/fonttools/fonttools/blob/03a3c8ed9e/Lib/fontTools/varLib/models.py#L47
fn normalize_value_with_extrapolation(val: f64, tuple: (i64, i64, i64)) -> NormalizedCoord {
    let (lower, default, upper) = tuple;
    let (lower, default, upper) = (lower as f64, default as f64, upper as f64);
    assert!(lower <= default);
    assert!(default <= upper);
    if val == default || lower == upper {
        return NormalizedCoord::default();
    }

    let val = if (val < default && lower != default) || (val > default && upper == default) {
        (val - default) / (default - lower)
    } else {
        assert!((val > default && upper != default) || (val < default && lower == default));
        (val - default) / (upper - default)
    };
    NormalizedCoord::new(val)
}

impl Shape {
    fn apply_affine(&mut self, affine: Affine) {
        match self {
            Shape::Path(path) => {
                path.nodes
                    .iter_mut()
                    .for_each(|node| node.apply_affine(affine));
            }
            Shape::Component(component) => {
                component.transform *= affine;
            }
        }
    }
}

impl Node {
    fn apply_affine(&mut self, affine: Affine) {
        self.pt = affine * self.pt;
    }
}

//https://github.com/googlefonts/glyphsLib/blob/52c982399b/Lib/glyphsLib/builder/smart_components.py#L43
fn normalized_location(
    for_layer: &Layer,
    base_layer: &Layer,
    name_to_tag_map: &BTreeMap<SmolStr, Tag>,
) -> Result<NormalizedLocation, BadSmartComponent> {
    for_layer
        .smart_component_positions
        .iter()
        .map(|(axis, pos)| {
            let tag = *name_to_tag_map
                .get(axis)
                .ok_or_else(|| BadSmartComponent::UnknownAxis(axis.clone()))?;
            let base_value = base_layer.smart_component_positions.get(axis).copied();
            let pos = match (base_value, *pos) {
                (Some(base), this) if base == this => NormalizedCoord::new(0.0),
                (None | Some(AxisPole::Min), AxisPole::Max) => NormalizedCoord::MAX,
                (None | Some(AxisPole::Max), AxisPole::Min) => NormalizedCoord::MIN,
                (base, this) => {
                    return Err(BadSmartComponent::StrangeAxisMapping {
                        axis: axis.clone(),
                        base,
                        child: this,
                    });
                }
            };
            Ok((tag, pos))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use kurbo::{Rect, Shape as _};

    use super::*;
    use crate::{Path, Shape};

    // ported from python:
    //https://github.com/googlefonts/glyphsLib/blob/52c982399b/tests/smart_components_test.py#L37
    fn smart_glyphs(master_id: &str) -> BTreeMap<SmolStr, Glyph> {
        fn rectangle_path(x: f64, y: f64, width: f64, height: f64) -> Path {
            fn line_to(x: f64, y: f64) -> Node {
                Node {
                    pt: (x, y).into(),
                    node_type: crate::NodeType::Line,
                }
            }
            Path {
                nodes: vec![
                    line_to(x, y),
                    line_to(x + width, y),
                    line_to(x + width, y + height),
                    line_to(x, y + height),
                ],
                closed: true,
                ..Default::default()
            }
        }
        let mut glyphs = BTreeMap::new();

        // Rectangle smart component
        let mut rectangle = Glyph {
            name: "_part.rectangle".into(),
            ..Default::default()
        };

        // Three axes
        rectangle
            .smart_component_axes
            .insert(SmolStr::new("Width"), 0..=1);
        rectangle
            .smart_component_axes
            .insert(SmolStr::new("Height"), 100..=500);
        rectangle
            .smart_component_axes
            .insert(SmolStr::new("Shift"), -100..=0);

        // Four layers
        let regular = Layer {
            layer_id: master_id.into(),
            width: 300.0.into(),
            shapes: vec![Shape::Path(rectangle_path(100.0, 100.0, 100.0, 100.0))],
            smart_component_positions: [
                (SmolStr::new("Width"), AxisPole::Min), // 1 is bottom pole
                (SmolStr::new("Height"), AxisPole::Min),
                (SmolStr::new("Shift"), AxisPole::Max), // 2 is the top pole
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let wide = Layer {
            layer_id: "wide".into(),
            associated_master_id: Some(master_id.into()),
            width: 700.0.into(),
            shapes: vec![Shape::Path(rectangle_path(100.0, 100.0, 500.0, 100.0))],
            smart_component_positions: [
                (SmolStr::new("Width"), AxisPole::Max),
                (SmolStr::new("Height"), AxisPole::Min),
                (SmolStr::new("Shift"), AxisPole::Max),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let tall = Layer {
            layer_id: "tall".into(),
            associated_master_id: Some(master_id.into()),
            width: 300.0.into(),
            shapes: vec![Shape::Path(rectangle_path(100.0, 100.0, 100.0, 500.0))],
            smart_component_positions: [
                (SmolStr::new("Width"), AxisPole::Min),
                (SmolStr::new("Height"), AxisPole::Max),
                (SmolStr::new("Shift"), AxisPole::Max),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let shifted = Layer {
            layer_id: "shifted".into(),
            associated_master_id: Some(master_id.into()),
            width: 100.0.into(),
            shapes: vec![Shape::Path(rectangle_path(0.0, 0.0, 100.0, 100.0))],
            smart_component_positions: [
                (SmolStr::new("Width"), AxisPole::Min),
                (SmolStr::new("Height"), AxisPole::Min),
                (SmolStr::new("Shift"), AxisPole::Min),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        rectangle.layers.extend([regular, wide, tall, shifted]);
        glyphs.insert(rectangle.name.clone(), rectangle);

        // Normal glyph "a" with a component
        let mut a = Glyph {
            name: "a".into(),
            ..Default::default()
        };

        let a_layer = Layer {
            layer_id: master_id.into(),
            width: 1000.0.into(),
            shapes: vec![Shape::Component(Component {
                name: SmolStr::new("_part.rectangle"),
                smart_component_values: BTreeMap::from([("derp".into(), 52.2)]),
                ..Default::default()
            })],
            ..Default::default()
        };

        a.layers.push(a_layer);
        glyphs.insert(a.name.clone(), a);

        glyphs
    }

    #[derive(Debug, PartialEq)]
    enum PathDirection {
        Clockwise,
        Otherwise,
    }

    fn get_rectangle_data(shape: &Shape) -> (Rect, PathDirection) {
        let shape = shape.as_path().unwrap();
        let nodes = &shape.nodes;
        let init = Rect::from_points(nodes[0].pt, nodes[0].pt);
        let rect = nodes.iter().fold(init, |acc, node| acc.union_pt(node.pt));

        let as_path = rect.to_path(0.1);
        let dir = if as_path.winding(rect.center()) < 0 {
            PathDirection::Clockwise
        } else {
            PathDirection::Otherwise
        };
        (rect, dir)
    }

    //https://github.com/googlefonts/glyphsLib/blob/52c982399b/tests/smart_components_test.py#L159
    #[test]
    fn test_smart_component_regular() {
        let master_id = "master01";
        let test_cases = [
            // Eight corners
            (
                [("Width", 0.0), ("Height", 100.0), ("Shift", 0.0)].as_slice(),
                (100.0, 100.0, 100.0, 100.0),
            ),
            (
                [("Width", 1.0), ("Height", 100.0), ("Shift", 0.0)].as_slice(),
                (100.0, 100.0, 500.0, 100.0),
            ),
            (
                [("Width", 0.0), ("Height", 500.0), ("Shift", 0.0)].as_slice(),
                (100.0, 100.0, 100.0, 500.0),
            ),
            (
                [("Width", 1.0), ("Height", 500.0), ("Shift", 0.0)].as_slice(),
                (100.0, 100.0, 500.0, 500.0),
            ),
            (
                [("Width", 0.0), ("Height", 100.0), ("Shift", -100.0)].as_slice(),
                (0.0, 0.0, 100.0, 100.0),
            ),
            (
                [("Width", 1.0), ("Height", 100.0), ("Shift", -100.0)].as_slice(),
                (0.0, 0.0, 500.0, 100.0),
            ),
            (
                [("Width", 0.0), ("Height", 500.0), ("Shift", -100.0)].as_slice(),
                (0.0, 0.0, 100.0, 500.0),
            ),
            (
                [("Width", 1.0), ("Height", 500.0), ("Shift", -100.0)].as_slice(),
                (0.0, 0.0, 500.0, 500.0),
            ),
            // Some points in the middle
            (
                [("Width", 0.5), ("Height", 300.0), ("Shift", -50.0)].as_slice(),
                (50.0, 50.0, 300.0, 300.0),
            ),
            // Extrapolation
            // NOTE: this currently fails. Does our variation model support extrapolation?
            //(
            //[("Width", 0.0), ("Height", 800.0), ("Shift", 0.0)].as_slice(),
            //(100.0, 100.0, 100.0, 800.0),
            //),
        ];

        let glyphs = smart_glyphs(master_id);
        let a_glyph = glyphs.get(&SmolStr::new("a")).unwrap();
        let a_layer = &a_glyph.layers[0];
        let component = a_layer.shapes[0].as_smart_component().unwrap();

        for (location, expected) in test_cases {
            // Set smart component values
            let mut modified_component = component.clone();
            modified_component.smart_component_values = location
                .iter()
                .map(|(k, v)| (SmolStr::new(k), *v))
                .collect();

            let rectangle = glyphs.get(&SmolStr::new("_part.rectangle")).unwrap();
            let shapes = instantiate_for_layer(master_id, &modified_component, rectangle)
                .expect("instantiate should succeed");

            let (rect, dir) = get_rectangle_data(&shapes[0]);
            let (x, y, w, h) = expected;
            let expected = Rect::new(x, y, x + w, y + h);

            assert_eq!(rect, expected, "Failed for values: {:?}", location);
            assert_eq!(
                dir,
                PathDirection::Otherwise,
                "Expected counter-clockwise winding for values: {:?}",
                location
            );
        }
    }
}
