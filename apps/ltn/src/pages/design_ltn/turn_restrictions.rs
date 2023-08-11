use geom::Distance;
use map_model::{PathV2, RoadID};
use osm2streets::{Direction, RestrictionType};
use widgetry::mapspace::{World, WorldOutcome, ObjectID};
use widgetry::{Color, EventCtx, GeomBatch, Key, Line, Text, TextExt, Widget};

use super::{road_name, EditMode, EditOutcome, Obj};
use crate::logic::turn_restrictions::destination_roads;
use crate::render::colors;
use crate::{App, Neighbourhood};

use super::shortcuts::FocusedRoad;

pub fn widget(ctx: &mut EventCtx, app: &App, focus: Option<&FocusedRoad>) -> Widget {
    match focus {
        Some(focus) => Widget::col(vec![format!(
            "Turn Restrictions from {}",
            app.per_map
                .map
                .get_r(focus.r)
                .get_name(app.opts.language.as_ref()),
        )
        .text_widget(ctx)]),
        None => Widget::nothing(),
    }
}

pub fn make_world(
    ctx: &mut EventCtx,
    app: &App,
    neighbourhood: &Neighbourhood,
    focus: &Option<FocusedRoad>,
) -> World<Obj> {
    let map = &app.per_map.map;
    let mut world = World::new();
    let focused_road = focus.as_ref().map(|f| f.r);

    let mut restricted_destinations: Vec<&RoadID> = Vec::new();
    let mut possible_destinations: Vec<RoadID> = Vec::new();
    if focused_road.is_some() {
        let focused_r = map.get_r(focused_road.unwrap());
        for (restriction, r2) in &focused_r.turn_restrictions {
            if *restriction == RestrictionType::BanTurns {
                restricted_destinations.push(r2);
            }
        }
        for (via, r2) in &focused_r.complicated_turn_restrictions {
            // TODO Show the 'via'? Or just draw the entire shape?
            restricted_destinations.push(via);
            restricted_destinations.push(r2);
        }

        // Account for one way streets when determining possible destinations
        // TODO This accounts for the oneway direction of the source street,
        // but not the oneway direction of the destination street
        possible_destinations = destination_roads(map, focused_r.id);
    }

    println!("TURN RESTRICTIONS: make_world :{:?}", focused_road);
    println!(
        "TURN RESTRICTIONS: restricted_destinations :{:?}",
        restricted_destinations.clone()
    );
    println!(
        "TURN RESTRICTIONS: possible_destinations :{:?}",
        possible_destinations.clone()
            .iter()
            .map(|r| map.get_r(*r).get_name(app.opts.language.as_ref()))
            .collect::<Vec<String>>()
    );

    println!(
        "TURN RESTRICTIONS: connected_exterior_roads :{:?}",
        &neighbourhood.connected_exterior_roads.len()
    );

    let all_r_id = [
        &neighbourhood.perimeter_roads,
        &neighbourhood.interior_roads,
        &neighbourhood.connected_exterior_roads,
    ]
    .into_iter()
    .flatten();

    for r in all_r_id {
        // for r in &neighbourhood.interior_roads {
        let road = map.get_r(*r);
        if focused_road == Some(*r) {
            let mut batch = GeomBatch::new();
            // Create a single compound geometry which represents a Road *and its connected roads* and draw
            // that geom as the mouseover geom for the Road. This avoids needing to update the representation of 
            // any Roads other then FocusedRoad.
            // Add focus road segment itself
            batch.push(
                Color::BLUE,
                road.get_thick_polygon().to_outline(Distance::meters(3.0)),
            );

            // Add possible destinations
            for possible_r in possible_destinations.clone() {
                let possible_road = map.get_r(possible_r);
                batch.push(
                    Color::YELLOW,
                    possible_road.get_thick_polygon().to_outline(Distance::meters(3.0)),
                );
            }

            // Add restricted_destinations
            for restricted_r in restricted_destinations.clone() {
                let restricted_road = map.get_r(*restricted_r);
                batch.push(
                    Color::RED,
                    restricted_road.get_thick_polygon().to_outline(Distance::meters(3.0)),
                );
            }

            world
                .add(Obj::Road(*r))
                .hitbox(road.get_thick_polygon())
                .draw(batch)
                .build(ctx);
        } else {
            world
                .add(Obj::Road(*r))
                .hitbox(road.get_thick_polygon())
                .drawn_in_master_batch()
                //.invisibly_hoverable()
                //.draw_color(colors::LOCAL_ROAD_LABEL.invert())
                .hover_color(colors::HOVER)
                .tooltip(Text::from(format!("{}", road_name(app, road))))
                .clickable()
                .build(ctx);
        }
    }

    // TODO
    // Highlight the current prohibited destination roads
    // Highlight the potential prohibited destination roads

    world.initialize_hover(ctx);
    world
}

pub fn handle_world_outcome(
    app: &mut App,
    outcome: WorldOutcome<Obj>,
    neighbourhood: &Neighbourhood,
) -> EditOutcome {
    // println!("TURN RESTRICTIONS: handle_world_outcome");

    match outcome {
        WorldOutcome::ClickedObject(Obj::Road(r)) => {
            // TODO - add logic based on which raod is clicked
            // Check if the ClickedObject is already highlighted
            // If so, then we should unhighlight it
            // If not and is one of the current prohibited destination roads,
            //      then we should remove that prohibited turn
            // If not and is one of the potential prohibited destination roads,
            //      then we should add that prohibited turn

            let subset = neighbourhood.shortcuts.subset(neighbourhood, r);
            app.session.edit_mode = EditMode::TurnRestrictions(Some(FocusedRoad {
                r,
                paths: subset.paths,
                current_idx: 0,
            }));
            println!("TURN RESTRICTIONS: handle_world_outcome - updatepanelandworld");
            EditOutcome::UpdatePanelAndWorld
        }
        WorldOutcome::ClickedFreeSpace(_) => {
            app.session.edit_mode = EditMode::TurnRestrictions(None);
            EditOutcome::UpdatePanelAndWorld
        }

        WorldOutcome::HoverChanged(before, after) => {
            handle_hover_change(before, after, app, neighbourhood)
        }

        _ => {
            // println!("TURN RESTRICTIONS: handle_world_outcome - NOTHING");
            EditOutcome::Nothing
        }
    }
}

fn handle_hover_change(before: Option<Obj>, after: Option<Obj>, app: &mut App, neighbourhood: &Neighbourhood) -> EditOutcome {

    println!("TURN RESTRICTIONS: handle_world_outcome before {:?}, after {:?}", before, after);
    match after {
        None => {
            // TODO Unsure why this isn't triggered when the mouse moves off the Road.
            // Need to investigate
            app.session.edit_mode = EditMode::TurnRestrictions(None);
            println!("TURN RESTRICTIONS: handle_world_outcome - No road highlighted");
            EditOutcome::UpdateAll
        }
        Some(Obj::Road(new_r)) => {
            // TODO Fixme. At present I'm just doing this so that I can reuse the FocusedRoad
            // struct here. I suspect that it would be better to create a single path which artificially
            // runs from one end of the FocusRoad to the other.
            let subset = neighbourhood.shortcuts.subset(neighbourhood, new_r);
            app.session.edit_mode = EditMode::TurnRestrictions(Some(FocusedRoad {
                r: new_r,
                paths: subset.paths,
                current_idx: 0,
            }));
            println!("TURN RESTRICTIONS: handle_world_outcome - new road highlighted");
            EditOutcome::UpdateAll
        }
        _ => {
            app.session.edit_mode = EditMode::TurnRestrictions(None);
            println!("TURN RESTRICTIONS: handle_world_outcome - catch other hover changes");
            EditOutcome::UpdateAll
        }
    }
}
