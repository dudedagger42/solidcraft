//! Fan studies and fan design (the Simulation workspace's fan tool): `sim.fan.design` turns a duty
//! (airflow, pressure rise, speed, size) into a fan's type, size, blades and the torque and power
//! its shaft needs; `sim.fan.run` spins a body of the design (a hand-drawn impeller, or one built
//! from a design) at a set speed inside its housing and reports the torque its motor must supply,
//! the shaft power and the air it moves. Neither touches the design or its undo history.

use serde_json::{Value, json};
use solidcraft_sim as sim;

use super::CommandSpec;
use crate::params::{bad, num, str_, vec3};
use crate::{Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("sim.fan.design", "Fan Design", fan_design).noundo().icon("sim_flow").params(
        "flow: airflow, m³/s (or flow_cfm: CFM, or flow_lps: L/s); pressure: total pressure rise, Pa; rpm: speed; max_diameter?: mm (the largest outer diameter allowed); kind?: axial|radial (default: picked from the specific speed); efficiency?: 0..1 (default 0.5: the total efficiency the shaft power is quoted at); density?: kg/m³ (default 1.204, air at 20 °C). Returns the type, diameters (mm), blade count, the axial blade's sections hub to tip (radius mm, inlet/outlet flow angles, stagger and camber in degrees, solidity, chord mm, de Haller's ratio, diffusion factor) or the radial impeller's outlet width (mm), blade angles and slip factor, and the shaft torque (N·m) and power (W), with notes where the duty pushes past the usual design rules",
    ),
    CommandSpec::new("sim.fan.compressor", "Compressor Design", compressor).noundo().icon("sim_flow").params(
        "flow: m³/s (or flow_cfm, flow_lps); pressure: total pressure rise over all stages, Pa; tip_diameter: mm (the rotor blades' tips); hub_ratio?: hub over tip diameter (default: searched, 0.4–0.75); stages?: count (default: the fewest that work); max_rpm?: (default 60000); efficiency?: 0..1 (default 0.6). A multi-stage axial compressor of repeating stages (free vortex): the speed, hub, stages and mid-span reaction are searched so every blade row keeps de Haller's ratio at 0.72. Returns the speed, diameters (mm), axial speed, blade counts and every row's sections hub to tip (radius mm, flow angles, stagger, camber in degrees, solidity, chord mm, de Haller's ratio, diffusion factor): rotor, stator (between stages), inlet vanes (when the stages take pre-swirl) and the two exit rows, with the torque (N·m) and shaft power (W)",
    ),
    CommandSpec::new("sim.fan.run", "Run Fan Study", fan_run).noundo().icon("sim_flow").params(
        "body: the impeller: a body of the design, hand drawn or built, or a list of bodies that turn together (a hub and its blade rows); axis: {origin: [x,y,z], dir: [x,y,z]} (mm; the impeller turns right-handed about dir at a positive rpm); rpm; housing?: [body names] (static walls around it: a shroud, a scroll); cell?: mm (default: a 48th of the impeller's diameter); pad?: mm of air past the bodies (default a third of the diameter); revolutions?: turns to run (default 3; the last one is averaged); turbulence?: Smagorinsky constant (default 0.15; 0 laminar); initial_speed?: m/s along dir at the start (default 0; near the speed it settles at, a ducted run settles in fewer turns, and its inlet holds that speed through the two-turn soft start before letting go, so a multistage compressor is not stalled by its slow first turns); back_pressure?: Pa: ducted, only the box's two faces across the axis open (dir must run along x, y or z; the casing should run end to end), the one dir points to held at this gauge pressure and the other at ambient, so the flow reported is what the fan delivers against it. Runs to the end before replying (CPU). Returns the motor torque (N·m, positive: along the turning), shaft power (W), the airflow through the impeller's disc along dir (m³/s, CFM; negative: against dir), the tip speed, cells and time steps",
    ),
];

/// Said with every fan result: the design rules and the study are not validated for engineering.
const NOTICE: &str = "Experimental: not validated for engineering use. Don't base design, safety or certification decisions on these results; confirm anything that matters with a validated solver or a physical test.";

fn positive(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x > 0.0)
}

fn fan_design(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sim.fan.design";
    let flow = positive(num(p, "flow"))
        .or_else(|| positive(num(p, "flow_cfm")).map(|c| c / sim::study::CFM_PER_M3S))
        .or_else(|| positive(num(p, "flow_lps")).map(|l| l / 1000.0))
        .ok_or_else(|| bad(cmd, "`flow` (m³/s), `flow_cfm` or `flow_lps` must be a positive number"))?;
    let pressure = positive(num(p, "pressure")).ok_or_else(|| bad(cmd, "`pressure` (Pa) must be a positive number"))?;
    let rpm = positive(num(p, "rpm")).ok_or_else(|| bad(cmd, "`rpm` must be a positive number"))?;
    let kind = match str_(p, "kind") {
        None => None,
        Some(k) if k.eq_ignore_ascii_case("axial") => Some(sim::fan_design::Kind::Axial),
        Some(k) if k.eq_ignore_ascii_case("radial") => Some(sim::fan_design::Kind::Radial),
        Some(_) => return Err(bad(cmd, "`kind` is axial or radial")),
    };
    let duty = sim::fan_design::Duty {
        flow,
        pressure,
        rpm,
        density: num(p, "density").unwrap_or(sim::study::AIR.density),
        max_diameter: num(p, "max_diameter").map(|d| d / 1000.0),
        kind,
        efficiency: num(p, "efficiency").unwrap_or(0.5),
    };
    let d = sim::fan_design::design(&duty).map_err(|e| bad(cmd, e.to_string()))?;
    let mm = |m: f64| (m * 1e4).round() / 10.0;
    let r3 = |x: f64| (x * 1e3).round() / 1e3;
    let mut out = json!({
        "kind": match d.kind { sim::fan_design::Kind::Axial => "axial", sim::fan_design::Kind::Radial => "radial" },
        "specific_speed": r3(d.specific_speed),
        "pressure_coefficient": r3(d.pressure_coefficient),
        "flow_coefficient": r3(d.flow_coefficient),
        "diameter_mm": mm(d.diameter),
        "tip_speed_ms": r3(d.tip_speed),
        "blades": d.blades,
        "torque_nm": d.torque,
        "power_w": r3(d.power),
        "notes": d.notes,
        "notice": NOTICE,
    });
    if let Some(o) = out.as_object_mut() {
        match d.kind {
            sim::fan_design::Kind::Axial => {
                o.insert("hub_diameter_mm".into(), json!(mm(d.inner_diameter)));
                o.insert(
                    "sections".into(),
                    json!(
                        d.sections
                            .iter()
                            .map(|s| json!({
                                "radius_mm": mm(s.radius),
                                "beta_in_deg": r3(s.beta_in),
                                "beta_out_deg": r3(s.beta_out),
                                "stagger_deg": r3(s.stagger),
                                "camber_deg": r3(s.camber),
                                "solidity": r3(s.solidity),
                                "chord_mm": mm(s.chord),
                                "de_haller": r3(s.de_haller),
                                "diffusion_factor": r3(s.diffusion),
                            }))
                            .collect::<Vec<_>>()
                    ),
                );
            }
            sim::fan_design::Kind::Radial => {
                o.insert("eye_diameter_mm".into(), json!(mm(d.inner_diameter)));
                o.insert("outlet_width_mm".into(), json!(mm(d.outlet_width)));
                o.insert("blade_outlet_angle_deg".into(), json!(r3(d.beta_outlet)));
                o.insert("blade_inlet_angle_deg".into(), json!(r3(d.beta_inlet)));
                o.insert("slip_factor".into(), json!(r3(d.slip)));
            }
        }
    }
    Ok(out)
}

fn compressor(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sim.fan.compressor";
    let flow = positive(num(p, "flow"))
        .or_else(|| positive(num(p, "flow_cfm")).map(|c| c / sim::study::CFM_PER_M3S))
        .or_else(|| positive(num(p, "flow_lps")).map(|l| l / 1000.0))
        .ok_or_else(|| bad(cmd, "`flow` (m³/s), `flow_cfm` or `flow_lps` must be a positive number"))?;
    let pressure = positive(num(p, "pressure")).ok_or_else(|| bad(cmd, "`pressure` (Pa) must be a positive number"))?;
    let tip = positive(num(p, "tip_diameter")).ok_or_else(|| bad(cmd, "`tip_diameter` (mm) must be a positive number"))?;
    let stages = match num(p, "stages") {
        None => None,
        Some(n) if n.fract() == 0.0 && (1.0..=20.0).contains(&n) => Some(n as usize),
        Some(_) => return Err(bad(cmd, "`stages` is a whole number 1…20")),
    };
    let duty = sim::fan_design::CompressorDuty {
        flow,
        pressure,
        density: num(p, "density").unwrap_or(sim::study::AIR.density),
        tip_diameter: tip / 1000.0,
        hub_ratio: num(p, "hub_ratio"),
        stages,
        max_rpm: num(p, "max_rpm").unwrap_or(60_000.0),
        efficiency: num(p, "efficiency").unwrap_or(0.6),
    };
    let c = sim::fan_design::compressor(&duty).map_err(|e| bad(cmd, e.to_string()))?;
    let r3 = |x: f64| (x * 1e3).round() / 1e3;
    let mm = |m: f64| (m * 1e4).round() / 10.0;
    let rotor = |s: &sim::fan_design::Section| json!({"radius_mm": mm(s.radius), "beta_in_deg": r3(s.beta_in), "beta_out_deg": r3(s.beta_out), "stagger_deg": r3(s.stagger), "camber_deg": r3(s.camber), "solidity": r3(s.solidity), "chord_mm": r3(s.chord * 1e3), "de_haller": r3(s.de_haller), "diffusion_factor": r3(s.diffusion)});
    let vane = |s: &sim::fan_design::VaneSection| json!({"radius_mm": mm(s.radius), "alpha_in_deg": r3(s.alpha_in), "alpha_out_deg": r3(s.alpha_out), "stagger_deg": r3(s.stagger), "camber_deg": r3(s.camber), "solidity": r3(s.solidity), "chord_mm": r3(s.chord * 1e3), "de_haller": r3(s.de_haller), "diffusion_factor": r3(s.diffusion)});
    Ok(json!({
        "notice": NOTICE,
        "stages": c.stages,
        "rpm": c.rpm.round(),
        "tip_diameter_mm": mm(c.tip_diameter),
        "hub_diameter_mm": mm(c.hub_diameter),
        "tip_speed_ms": r3(c.tip_speed),
        "axial_speed_ms": r3(c.axial_speed),
        "flow_coefficient": r3(c.flow_coefficient),
        "stage_loading": r3(c.stage_loading),
        "reaction": r3(c.reaction),
        "rotor_blades": c.rotor_blades,
        "stator_vanes": c.stator_vanes,
        "inlet_vanes": c.inlet_vanes,
        "exit_vanes": c.exit_vanes,
        "rotor": c.rotor.iter().map(rotor).collect::<Vec<_>>(),
        "stator": c.stator.iter().map(vane).collect::<Vec<_>>(),
        "inlet": c.inlet.iter().map(vane).collect::<Vec<_>>(),
        "exit": c.exit.iter().map(|row| row.iter().map(vane).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "stage_pressure_pa": r3(c.stage_pressure),
        "torque_nm": c.torque,
        "power_w": r3(c.power),
        "notes": c.notes,
    }))
}

fn fan_run(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "sim.fan.run";
    let bodies: Vec<String> = match p.get("body") {
        Some(Value::String(b)) => vec![b.clone()],
        Some(Value::Array(a)) if !a.is_empty() => a
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(cmd, "`body` names the impeller (or lists its parts)")))
            .collect::<Result<_>>()?,
        _ => return Err(bad(cmd, "`body` names the impeller (or lists its parts)")),
    };
    let body = bodies.join(", ");
    let axis = p.get("axis").ok_or_else(|| bad(cmd, "`axis` is {origin: [x,y,z], dir: [x,y,z]}"))?;
    let origin = axis.get("origin").and_then(vec3).ok_or_else(|| bad(cmd, "`axis.origin` must be [x,y,z]"))?;
    let dir = axis.get("dir").and_then(vec3).ok_or_else(|| bad(cmd, "`axis.dir` must be [x,y,z]"))?;
    let rpm = num(p, "rpm").filter(|r| r.is_finite() && *r != 0.0 && r.abs() <= 1e6).ok_or_else(|| bad(cmd, "`rpm` must be a non-zero number"))?;
    let housing: Vec<String> = match p.get("housing") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => {
            a.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(cmd, "`housing` lists body names"))).collect::<Result<_>>()?
        }
        Some(_) => return Err(bad(cmd, "`housing` lists body names")),
    };
    let st = s.world_state();
    let impeller: Vec<_> =
        bodies.iter().map(|b| st.body(b).map(|x| x.mesh()).ok_or_else(|| bad(cmd, format!("no body `{b}`")))).collect::<Result<_>>()?;
    let walls: Vec<_> =
        housing.iter().map(|h| st.body(h).map(|b| b.mesh()).ok_or_else(|| bad(cmd, format!("no body `{h}`")))).collect::<Result<_>>()?;
    let unit = dir.normalized().ok_or_else(|| bad(cmd, "`axis.dir` must not be zero"))?;
    let radius = impeller
        .iter()
        .flat_map(|m| m.positions.iter())
        .map(|q| {
            let d = *q - origin;
            (d - unit * d.dot(unit)).len()
        })
        .fold(0.0f64, f64::max);
    if !(radius > 0.0) {
        return Err(bad(cmd, "the impeller has no extent about the axis"));
    }
    let cell = match num(p, "cell") {
        Some(c) if c.is_finite() && c > 0.0 => c,
        Some(_) => return Err(bad(cmd, "`cell` must be a positive length (mm)")),
        None => 2.0 * radius / 48.0,
    };
    let pad = num(p, "pad").filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(2.0 * radius / 3.0);
    let revolutions = match num(p, "revolutions") {
        None => 3.0,
        Some(v) if v.is_finite() && (0.1..=200.0).contains(&v) => v,
        Some(_) => return Err(bad(cmd, "`revolutions` is 0.1…200")),
    };
    let turbulence = match num(p, "turbulence") {
        None => 0.15,
        Some(v) if v.is_finite() && (0.0..=0.5).contains(&v) => v,
        Some(_) => return Err(bad(cmd, "`turbulence` is a Smagorinsky constant, 0…0.5")),
    };
    let initial_speed = match num(p, "initial_speed") {
        None => 0.0,
        Some(v) if v.is_finite() && v.abs() < 200.0 => v,
        Some(_) => return Err(bad(cmd, "`initial_speed` is m/s along dir")),
    };
    let duct = match num(p, "back_pressure") {
        Some(v) if v.is_finite() => Some(v),
        Some(_) => return Err(bad(cmd, "`back_pressure` is a gauge pressure in Pa")),
        None => None,
    };
    let setup = sim::FanSetup { cell, pad, air: sim::study::AIR, rpm, origin, axis: dir, turbulence, max_cells: 50_000_000, duct, initial_speed };
    let refs: Vec<&solidcraft_geom::Mesh> = walls.iter().map(|m| m.as_ref()).collect();
    let parts: Vec<&solidcraft_geom::Mesh> = impeller.iter().map(|m| m.as_ref()).collect();
    let mut study = sim::prepare_fan(&refs, &parts, &setup).map_err(|e| bad(cmd, e.to_string()))?;
    let cells = study.grid.len();
    let r = study.run(revolutions, |_| true);
    if ![r.torque, r.power, r.flow].iter().all(|v| v.is_finite()) {
        return Err(bad(
            cmd,
            format!(
                "the flow went unstable after {:.2} turns ({} cells of {cell} mm): give the air more room (`pad`) or use smaller cells",
                r.revolutions, cells
            ),
        ));
    }
    let warnings: Vec<String> = study.warnings.iter().map(|w| format!("{w:?}")).collect();
    Ok(json!({
        "notice": NOTICE,
        "body": body,
        "rpm": rpm,
        "torque_nm": r.torque,
        "power_w": r.power,
        "flow_m3s": r.flow,
        "flow_cfm": r.flow * sim::study::CFM_PER_M3S,
        "tip_speed_ms": r.tip_speed,
        "revolutions": r.revolutions,
        "air_time_s": r.time,
        "cell_mm": cell,
        "cells": cells,
        "steps_per_revolution": study.steps_per_revolution(),
        "warnings": warnings,
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use serde_json::json;

    use crate::Session;

    /// A printer's part-cooling duty in CFM designs an axial fan of a few centimetres, its
    /// sections hub to tip, the torque and power its shaft needs; a blower's duty a radial one.
    #[test]
    fn fan_design_answers_a_duty_with_a_fan() {
        let mut s = Session::default();
        let r = s.execute("sim.fan.design", &json!({"flow_cfm": 15, "pressure": 40, "rpm": 6000})).unwrap();
        assert_eq!(r["kind"], "axial", "{r}");
        assert_eq!(r["sections"].as_array().unwrap().len(), 5);
        let d = r["diameter_mm"].as_f64().unwrap();
        assert!(d > 20.0 && d < 80.0, "{r}");
        assert!(r["torque_nm"].as_f64().unwrap() > 0.0 && r["power_w"].as_f64().unwrap() > 0.0);
        let r = s.execute("sim.fan.design", &json!({"flow_lps": 5, "pressure": 400, "rpm": 4000})).unwrap();
        assert_eq!(r["kind"], "radial", "{r}");
        assert!(r["outlet_width_mm"].as_f64().unwrap() > 0.0);
        assert!(s.execute("sim.fan.design", &json!({"flow": 0.01, "pressure": 40})).is_err(), "rpm is needed");
        assert!(s.execute("sim.fan.design", &json!({"flow": 0.01, "pressure": 40, "rpm": 3000, "kind": "mixed"})).is_err());
    }

    /// A paddle drawn in the design, spun about z: the motor needs torque against the air.
    #[test]
    fn fan_run_spins_a_body_of_the_design() {
        let mut s = Session::default();
        s.execute("solid.box", &json!({"corner": [-10, -1, -2], "length": 20, "width": 2, "height": 4, "body_name": "Paddle"})).unwrap();
        let r = s
            .execute(
                "sim.fan.run",
                &json!({"body": "Paddle", "axis": {"origin": [0, 0, 0], "dir": [0, 0, 1]}, "rpm": 3000, "cell": 1.0, "pad": 12, "revolutions": 1.0}),
            )
            .unwrap();
        assert!(r["torque_nm"].as_f64().is_some_and(|t| t > 0.0), "{r}");
        assert!(r["power_w"].as_f64().unwrap() > 0.0, "{r}");
        assert!(s.execute("sim.fan.run", &json!({"body": "Nope", "axis": {"origin": [0, 0, 0], "dir": [0, 0, 1]}, "rpm": 3000})).is_err());
    }
}
