use std::collections::HashMap;

use geom::Distance;
use map_gui::tools::PopupMsg;
use map_model::osm::RoadRank;
use map_model::LaneType;
use widgetry::{
    ButtonBuilder, Color, ControlState, Drawable, EdgeInsets, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Panel, RewriteColor, Text, Toggle, VerticalAlignment,
    Widget,
};

use crate::app::{App, Transition};
use crate::ungap::bike_network;
use crate::ungap::bike_network::DrawNetworkLayer;
use crate::ungap::labels::DrawRoadLabels;

/// A bottom-right panel for managing a bunch of toggleable layers in the "ungap the map" tool.
pub struct Layers {
    panel: Panel,
    bike_network_layer: Option<DrawNetworkLayer>,
    labels: Option<DrawRoadLabels>,
    elevation: bool,
    steep_streets: Option<Drawable>,
    // TODO Once widgetry buttons can take custom enums, that'd be perfect here
    road_types: HashMap<String, Drawable>,

    zoom_enabled_cache_key: (bool, bool),
    map_edit_key: usize,
}

impl Layers {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Layers {
        Layers {
            panel: make_bottom_right_panel(ctx, app, true, true, false, false),
            bike_network_layer: Some(DrawNetworkLayer::new()),
            labels: Some(DrawRoadLabels::new()),
            elevation: false,
            steep_streets: None,
            road_types: HashMap::new(),
            zoom_enabled_cache_key: zoom_enabled_cache_key(ctx),
            map_edit_key: usize::MAX,
        }
    }

    fn highlight_road_type(&mut self, ctx: &mut EventCtx, app: &App, name: &str) {
        // TODO Button enums would rock
        if name == "bike network"
            || name == "road labels"
            || name == "elevation"
            || name == "steep streets"
            || name.starts_with("about ")
        {
            return;
        }
        if self.road_types.contains_key(name) {
            return;
        }

        let mut batch = GeomBatch::new();
        for r in app.primary.map.all_roads() {
            let rank = r.get_rank();
            let mut bike_lane = false;
            let mut buffer = false;
            for (_, _, lt) in r.lanes_ltr() {
                if lt == LaneType::Biking {
                    bike_lane = true;
                } else if matches!(lt, LaneType::Buffer(_)) {
                    buffer = true;
                }
            }

            let show = (name == "highway" && rank == RoadRank::Highway)
                || (name == "major street" && rank == RoadRank::Arterial)
                || (name == "minor street" && rank == RoadRank::Local)
                || (name == "trail" && r.is_cycleway())
                || (name == "protected bike lane" && bike_lane && buffer)
                || (name == "painted bike lane" && bike_lane && !buffer)
                || (name == "greenway" && bike_network::is_greenway(r));
            if show {
                // TODO If it's a bike element, should probably thicken for the unzoomed scale...
                // the maximum amount?
                batch.push(Color::CYAN, r.get_thick_polygon(&app.primary.map));
            }
        }

        self.road_types.insert(name.to_string(), ctx.upload(batch));
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        let key = app.primary.map.get_edits_change_key();
        if self.map_edit_key != key {
            self.map_edit_key = key;
            if self.bike_network_layer.is_some() {
                self.bike_network_layer = Some(DrawNetworkLayer::new());
            }
            self.road_types.clear();
        }

        if ctx.redo_mouseover() && self.elevation {
            let mut label = Text::new().into_widget(ctx);

            if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
                if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                    if let Some((elevation, _)) = app
                        .session
                        .elevation_contours
                        .value()
                        .unwrap()
                        .0
                        .closest_pt(pt, Distance::meters(300.0))
                    {
                        label =
                            Line(format!("{} ft", elevation.to_feet().round())).into_widget(ctx);
                    }
                }
            }
            self.panel.replace(ctx, "current elevation", label);
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                return Some(Transition::Push(match x.as_ref() {
                    // TODO Add physical picture examples
                    "highway" => PopupMsg::new_state(ctx, "Highways", vec!["Unless there's a separate trail (like on the 520 or I90 bridge), highways aren't accessible to biking"]),
                    "major street" => PopupMsg::new_state(ctx, "Major streets", vec!["Arterials have more traffic, but are often where businesses are located"]),
                    "minor street" => PopupMsg::new_state(ctx, "Minor streets", vec!["Local streets have a low volume of traffic and are usually comfortable for biking, even without dedicated infrastructure"]),
                    "trail" => PopupMsg::new_state(ctx, "Trails", vec!["Trails like the Burke Gilman are usually well-separated from vehicle traffic. The space is usually shared between people walking, cycling, and rolling."]),
                    "protected bike lane" => PopupMsg::new_state(ctx, "Protected bike lanes", vec!["Bike lanes separated from vehicle traffic by physical barriers or a few feet of striping"]),
                    "painted bike lane" => PopupMsg::new_state(ctx, "Painted bike lanes", vec!["Bike lanes without any separation from vehicle traffic. Often uncomfortably close to the \"door zone\" of parked cars."]),
                    "greenway" => PopupMsg::new_state(ctx, "Stay Healthy Streets and neighborhood greenways", vec!["Residential streets with additional signage and light barriers. These are intended to be low traffic, dedicated for people walking and biking."]),
                    // TODO Add URLs
                    "about the elevation data" => PopupMsg::new_state(ctx, "About the elevation data", vec!["Biking uphill next to traffic without any dedicated space isn't fun.", "Biking downhill next to traffic, especially in the door-zone of parked cars, and especially on Seattle's bumpy roads... is downright terrifying.", "", "Note the elevation data is incorrect near bridges.", "Thanks to King County LIDAR for the data, and Eldan Goldenberg for processing it."]),
                   "zoom map out" => {
                        ctx.canvas.center_zoom(-8.0);
                        debug!("clicked zoomed out to: {}", ctx.canvas.cam_zoom);
                        self.panel = make_bottom_right_panel(ctx, app, self.bike_network_layer.is_some(), self.labels.is_some(), self.elevation, self.steep_streets.is_some());
                        return Some(Transition::Keep);
                    },
                    "zoom map in" => {
                        ctx.canvas.center_zoom(8.0);
                        debug!("clicked zoomed in to: {}", ctx.canvas.cam_zoom);
                        self.panel = make_bottom_right_panel(ctx, app, self.bike_network_layer.is_some(), self.labels.is_some(), self.elevation, self.steep_streets.is_some());
                        return Some(Transition::Keep);
                    },
                    _ => unreachable!(),
            }));
            }
            Outcome::Changed(x) => match x.as_ref() {
                "bike network" => {
                    if self.panel.is_checked("bike network") {
                        self.bike_network_layer = Some(DrawNetworkLayer::new());
                    } else {
                        self.bike_network_layer = None;
                    }
                }
                "road labels" => {
                    if self.panel.is_checked("road labels") {
                        self.labels = Some(DrawRoadLabels::new());
                    } else {
                        self.labels = None;
                    }
                }
                "elevation" => {
                    self.elevation = self.panel.is_checked("elevation");
                    self.panel = make_bottom_right_panel(
                        ctx,
                        app,
                        self.bike_network_layer.is_some(),
                        self.labels.is_some(),
                        self.elevation,
                        self.steep_streets.is_some(),
                    );
                    if self.elevation {
                        let name = app.primary.map.get_name().clone();
                        if app.session.elevation_contours.key() != Some(name.clone()) {
                            let mut low = Distance::ZERO;
                            let mut high = Distance::ZERO;
                            for i in app.primary.map.all_intersections() {
                                low = low.min(i.elevation);
                                high = high.max(i.elevation);
                            }
                            // TODO Maybe also draw the uphill arrows on the steepest streets?
                            let value = crate::layer::elevation::ElevationContours::make_contours(
                                ctx, app, low, high,
                            );
                            app.session.elevation_contours.set(name, value);
                        }
                    }
                }
                "steep streets" => {
                    if self.panel.is_checked("steep streets") {
                        let (colorer, _, uphill_legend) =
                            crate::layer::elevation::SteepStreets::make_colorer(ctx, app);
                        // Make a horizontal legend for the incline
                        let mut legend: Vec<Widget> = colorer
                            .categories
                            .iter()
                            .map(|(label, color)| {
                                legend_batch(ctx, *color, Text::from(Line(label).fg(Color::WHITE)))
                                    .into_widget(ctx)
                            })
                            .collect();
                        legend.push(uphill_legend);
                        let legend = Widget::custom_row(legend);
                        self.panel.replace(ctx, "steep streets legend", legend);

                        self.steep_streets = Some(colorer.unzoomed.upload(ctx));
                    } else {
                        self.steep_streets = None;
                        self.panel.replace(
                            ctx,
                            "steep streets legend",
                            Text::new().into_widget(ctx),
                        );
                    }
                }
                _ => unreachable!(),
            },
            _ => {}
        }
        if let Some(name) = self.panel.currently_hovering().cloned() {
            self.highlight_road_type(ctx, app, &name);
        }

        if self.zoom_enabled_cache_key != zoom_enabled_cache_key(ctx) {
            // approriately disable/enable zoom buttons in case user scroll-zoomed
            self.panel = make_bottom_right_panel(
                ctx,
                app,
                self.bike_network_layer.is_some(),
                self.labels.is_some(),
                self.elevation,
                self.steep_streets.is_some(),
            );
            self.zoom_enabled_cache_key = zoom_enabled_cache_key(ctx);
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            if let Some(ref n) = self.bike_network_layer {
                n.draw(g, app);
            }
            if let Some(ref l) = self.labels {
                l.draw(g, app);
            }

            if self.elevation {
                if let Some((_, ref draw)) = app.session.elevation_contours.value() {
                    g.redraw(draw);
                }
            }
            if let Some(ref draw) = self.steep_streets {
                g.redraw(draw);
            }

            if let Some(name) = self.panel.currently_hovering() {
                if let Some(draw) = self.road_types.get(name) {
                    g.redraw(draw);
                }
            }
        }
    }
}

fn make_zoom_controls(ctx: &mut EventCtx) -> Widget {
    let builder = ctx
        .style()
        .btn_floating
        .btn()
        .image_dims(30.0)
        .outline((1.0, ctx.style().btn_plain.fg), ControlState::Default)
        .padding(12.0);

    Widget::custom_col(vec![
        builder
            .clone()
            .image_path("system/assets/speed/plus.svg")
            .corner_rounding(geom::CornerRadii {
                top_left: 16.0,
                top_right: 16.0,
                bottom_right: 0.0,
                bottom_left: 0.0,
            })
            .disabled(ctx.canvas.is_max_zoom())
            .build_widget(ctx, "zoom map in"),
        builder
            .image_path("system/assets/speed/minus.svg")
            .image_dims(30.0)
            .padding(12.0)
            .corner_rounding(geom::CornerRadii {
                top_left: 0.0,
                top_right: 0.0,
                bottom_right: 16.0,
                bottom_left: 16.0,
            })
            .disabled(ctx.canvas.is_min_zoom())
            .build_widget(ctx, "zoom map out"),
    ])
}

fn make_legend(
    ctx: &mut EventCtx,
    app: &App,
    bike_network: bool,
    labels: bool,
    elevation: bool,
    steep_streets: bool,
) -> Widget {
    Widget::col(vec![
        Widget::custom_row(vec![
            // TODO Looks too close to access restrictions
            legend_item(ctx, app.cs.unzoomed_highway, "highway"),
            legend_item(ctx, app.cs.unzoomed_arterial, "major street"),
            legend_item(ctx, app.cs.unzoomed_residential, "minor street"),
        ]),
        Widget::custom_row(vec![
            Toggle::checkbox(ctx, "bike network", Key::B, bike_network),
            legend_item(ctx, *bike_network::DEDICATED_TRAIL, "trail"),
            legend_item(
                ctx,
                *bike_network::PROTECTED_BIKE_LANE,
                "protected bike lane",
            ),
            legend_item(ctx, *bike_network::PAINTED_BIKE_LANE, "painted bike lane"),
            legend_item(ctx, *bike_network::GREENWAY, "greenway"),
        ]),
        // TODO Distinguish door-zone bike lanes?
        // TODO Call out bike turning boxes?
        // TODO Call out bike signals?
        Toggle::checkbox(ctx, "road labels", Key::L, labels),
        Widget::row(vec![
            Toggle::checkbox(ctx, "elevation", Key::E, elevation),
            ctx.style()
                .btn_plain
                .icon("system/assets/tools/info.svg")
                .build_widget(ctx, "about the elevation data")
                .centered_vert(),
            Text::new()
                .into_widget(ctx)
                .named("current elevation")
                .centered_vert(),
        ]),
        Widget::row(vec![
            Toggle::checkbox(ctx, "steep streets", Key::S, steep_streets),
            // A placeholder
            Text::new().into_widget(ctx).named("steep streets legend"),
        ]),
        // TODO Probably a collisions layer
    ])
}

fn make_bottom_right_panel(
    ctx: &mut EventCtx,
    app: &App,
    bike_network: bool,
    labels: bool,
    elevation: bool,
    steep_streets: bool,
) -> Panel {
    Panel::new_builder(Widget::col(vec![
        make_zoom_controls(ctx).align_right().padding_right(16),
        make_legend(ctx, app, bike_network, labels, elevation, steep_streets)
            .padding(16)
            .bg(ctx.style().panel_bg),
    ]))
    .aligned(HorizontalAlignment::Right, VerticalAlignment::Bottom)
    .build_custom(ctx)
}

fn legend_batch(ctx: &mut EventCtx, color: Color, txt: Text) -> GeomBatch {
    // TODO Height of the "trail" button is slightly too low!
    // Text with padding and a background color
    let (mut batch, hitbox) = txt
        .render(ctx)
        .batch()
        .container()
        .padding(EdgeInsets {
            top: 10.0,
            bottom: 10.0,
            left: 20.0,
            right: 20.0,
        })
        .into_geom(ctx, None);
    batch.unshift(color, hitbox);
    batch
}

fn legend_item(ctx: &mut EventCtx, color: Color, label: &str) -> Widget {
    let batch = legend_batch(ctx, color, Text::from(Line(label)));
    return ButtonBuilder::new()
        .custom_batch(batch.clone(), ControlState::Default)
        .custom_batch(
            batch.color(RewriteColor::Change(color, color.alpha(0.6))),
            ControlState::Hovered,
        )
        .build_widget(ctx, label);
}

fn zoom_enabled_cache_key(ctx: &EventCtx) -> (bool, bool) {
    (ctx.canvas.is_max_zoom(), ctx.canvas.is_min_zoom())
}
