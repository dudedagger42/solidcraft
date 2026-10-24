//! Fan design from a duty: the airflow and pressure rise wanted at a speed, within a size, become
//! a type (axial or radial), a diameter, blades (angles, count, chord, camber) and the torque and
//! power the shaft needs. Mean-line design, the textbook way (S. L. Dixon & C. A. Hall, *Fluid
//! Mechanics and Thermodynamics of Turbomachinery*, 7th ed., chs. 1–3 and 7):
//!
//! - **Dimensionless duty.** The specific work `Y = Δp / ρ`, the speed `ω`, the flow `Q`: the
//!   specific speed `Ωs = ω √Q / Y^¾` picks the type (axial above about 2.5, radial below about
//!   1.2, mixed between); the pressure coefficient `ψ = 2Y / u²` the type works at sets the tip
//!   speed `u` and so the diameter.
//! - **Euler's equation.** The work per mass is `u₂ c_u2 − u₁ c_u1` (no inlet swirl here, so
//!   `u₂ c_u2`); the shaft torque is `ρ Q r₂ c_u2` at the ideal work and the power `ρ Q Y / η`.
//! - **Axial blades, free vortex.** `r c_u2` is the same at every radius, the axial speed is the
//!   flow over the annulus; the relative flow angles at hub, middle and tip come from the
//!   velocity triangles. The blade's solidity follows from Lieblein's diffusion factor
//!   `DF = 1 − w₂/w₁ + Δc_u / (2 σ w₁)` held at 0.45, and de Haller's ratio `w₂/w₁` is reported
//!   (below 0.7 a stage stalls).
//! - **Radial impeller, backward-curved blades.** Wiesner's slip factor
//!   `σ_s = 1 − √(cos β₂) / Z^0.7` (β₂ from radial) lowers the swirl the blades give; the outlet
//!   width carries the flow at a meridional speed of a quarter of the tip speed, and the eye is
//!   sized for the least relative speed at the inlet tip.
//!
//! The efficiency is an assumption the design is quoted at, not a prediction: check a design by
//! running it as a [`crate::fan`] study.

use crate::{Result, SimError};

/// The kind of fan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Axial,
    Radial,
}

/// What a fan must do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Duty {
    /// Air through the fan, m³/s.
    pub flow: f64,
    /// Total pressure rise, Pa.
    pub pressure: f64,
    /// Speed, rpm.
    pub rpm: f64,
    /// Air density, kg/m³.
    pub density: f64,
    /// The largest outer diameter allowed, m (none: whatever the duty asks).
    pub max_diameter: Option<f64>,
    /// The type, or none to pick from the specific speed.
    pub kind: Option<Kind>,
    /// The total efficiency assumed for the shaft power (0..1].
    pub efficiency: f64,
}

/// One radius of an axial blade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    /// Radius, m.
    pub radius: f64,
    /// Relative flow angles at inlet and outlet, degrees from the axis.
    pub beta_in: f64,
    pub beta_out: f64,
    /// Stagger (the chord's angle from the axis) and camber (the turning of the camber line),
    /// degrees.
    pub stagger: f64,
    pub camber: f64,
    /// Solidity (chord over blade spacing) and chord, m.
    pub solidity: f64,
    pub chord: f64,
    /// De Haller's ratio w₂/w₁ and Lieblein's diffusion factor.
    pub de_haller: f64,
    pub diffusion: f64,
}

/// A designed fan.
#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub kind: Kind,
    /// Specific speed ω √Q / Y^¾ (dimensionless) and the pressure and flow coefficients.
    pub specific_speed: f64,
    pub pressure_coefficient: f64,
    pub flow_coefficient: f64,
    /// Outer (tip) diameter, m; hub or eye diameter, m.
    pub diameter: f64,
    pub inner_diameter: f64,
    pub tip_speed: f64,
    pub blades: usize,
    /// Axial: the blade's sections from hub to tip. Radial: none.
    pub sections: Vec<Section>,
    /// Radial: outlet width (m), outlet blade angle (degrees back from radial), inlet blade
    /// angle at the eye's tip (degrees from tangential) and the slip factor.
    pub outlet_width: f64,
    pub beta_outlet: f64,
    pub beta_inlet: f64,
    pub slip: f64,
    /// The swirl the blades give the air at the outer radius, m/s, and the ideal work, J/kg.
    pub swirl: f64,
    pub work: f64,
    /// Shaft torque (N·m) and power (W) at the assumed efficiency.
    pub torque: f64,
    pub power: f64,
    /// Things the duty pushes past the usual design rules.
    pub notes: Vec<String>,
}

/// The pressure coefficient 2Y/u² an axial fan is designed at (a radial one's follows from its
/// blade angle and slip).
const PSI_AXIAL: f64 = 0.35;

/// Design a fan for `duty`.
pub fn design(duty: &Duty) -> Result<Design> {
    let Duty { flow, pressure, rpm, density, max_diameter, kind, efficiency } = *duty;
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if !(positive(flow) && positive(pressure) && positive(rpm) && positive(density) && positive(efficiency) && efficiency <= 1.0) {
        return Err(SimError::BadDuty);
    }
    let omega = rpm * std::f64::consts::TAU / 60.0;
    let y = pressure / density;
    let ns = omega * flow.sqrt() / y.powf(0.75);
    let kind = kind.unwrap_or(if ns >= 1.8 { Kind::Axial } else { Kind::Radial });
    let mut notes = Vec::new();
    match kind {
        Kind::Axial if ns < 1.2 => {
            notes.push(format!("specific speed {ns:.2} is low for an axial fan (best above about 2.5): a radial fan suits this duty better"))
        }
        Kind::Radial if ns > 2.5 => {
            notes.push(format!("specific speed {ns:.2} is high for a radial fan (best below about 1.2): an axial fan suits this duty better"))
        }
        _ => {}
    }
    let mut d = match kind {
        Kind::Axial => axial(flow, y, omega, density, efficiency, &mut notes)?,
        Kind::Radial => radial(flow, y, omega, density, efficiency, &mut notes)?,
    };
    d.specific_speed = ns;
    if let Some(max) = max_diameter
        && d.diameter > max
    {
        // The speed that fits the size at this type's pressure coefficient.
        let need = rpm * d.diameter / max;
        notes.push(format!("needs a {:.1} mm diameter, over the {:.1} mm allowed: about {need:.0} rpm fits it", d.diameter * 1e3, max * 1e3));
    }
    d.notes = notes;
    Ok(d)
}

fn axial(flow: f64, y: f64, omega: f64, density: f64, efficiency: f64, notes: &mut Vec<String>) -> Result<Design> {
    // Tip speed from the pressure coefficient at the tip, the hub from the flow coefficient: a
    // hub ratio that keeps the axial speed near a third of the tip speed (small fans run about
    // 0.25–0.4), within 0.3–0.7.
    let work = y / efficiency.sqrt(); // the ideal work: part of the losses
    let tip_speed = (2.0 * work / PSI_AXIAL).sqrt();
    let r_tip = tip_speed / omega;
    let mut hub_ratio = 0.45;
    let mut cm = flow / (std::f64::consts::PI * r_tip * r_tip * (1.0 - hub_ratio * hub_ratio));
    for _ in 0..40 {
        let target = 0.33 * tip_speed;
        // More hub, less annulus, faster axial flow.
        let next = (1.0 - flow / (std::f64::consts::PI * r_tip * r_tip * target)).max(0.0).sqrt();
        hub_ratio = next.clamp(0.3, 0.7);
        cm = flow / (std::f64::consts::PI * r_tip * r_tip * (1.0 - hub_ratio * hub_ratio));
    }
    let r_hub = hub_ratio * r_tip;
    // Free vortex: r c_u2 = work / ω at every radius.
    let k = work / omega;
    let mut sections = Vec::new();
    let blades = blade_count(hub_ratio);
    for f in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let r = r_hub + f * (r_tip - r_hub);
        let u = omega * r;
        let cu2 = k / r;
        if cu2 >= u {
            notes.push(format!(
                "at radius {:.1} mm the swirl the duty needs ({cu2:.1} m/s) reaches the blade speed ({u:.1} m/s): more speed or a larger hub",
                r * 1e3
            ));
        }
        let w1 = (cm * cm + u * u).sqrt();
        let w2 = (cm * cm + (u - cu2).powi(2)).sqrt();
        let beta_in = (u / cm).atan();
        let beta_out = ((u - cu2) / cm).atan();
        let de_haller = w2 / w1;
        // Solidity for DF = 0.45, within 0.4–1.6.
        let solidity = (cu2 / (2.0 * w1 * (0.45 - 1.0 + de_haller).max(0.05))).clamp(0.4, 1.6);
        let diffusion = 1.0 - de_haller + cu2 / (2.0 * solidity * w1);
        let chord = solidity * std::f64::consts::TAU * r / blades as f64;
        // A circular-arc camber line about as turned as the flow, plus a little deviation
        // (Carter's rule, m ≈ 0.26 for a circular arc: δ = m θ / √σ).
        let turning = beta_in - beta_out;
        let camber = turning / (1.0 - 0.26 / solidity.sqrt());
        let stagger = 0.5 * (beta_in + beta_out);
        sections.push(Section {
            radius: r,
            beta_in: beta_in.to_degrees(),
            beta_out: beta_out.to_degrees(),
            stagger: stagger.to_degrees(),
            camber: camber.to_degrees(),
            solidity,
            chord,
            de_haller,
            diffusion,
        });
    }
    if let Some(hub) = sections.first()
        && hub.de_haller < 0.7
    {
        notes.push(format!(
            "de Haller's ratio at the hub is {:.2} (below 0.7 the blades stall there): lower the pressure per stage or raise the speed",
            hub.de_haller
        ));
    }
    let torque = density * flow * work / omega;
    Ok(Design {
        kind: Kind::Axial,
        specific_speed: 0.0,
        pressure_coefficient: PSI_AXIAL,
        flow_coefficient: cm / tip_speed,
        diameter: 2.0 * r_tip,
        inner_diameter: 2.0 * r_hub,
        tip_speed,
        blades,
        sections,
        outlet_width: 0.0,
        beta_outlet: 0.0,
        beta_inlet: 0.0,
        slip: 1.0,
        swirl: k / r_tip,
        work,
        torque: torque / efficiency.sqrt(),
        power: density * flow * y / efficiency,
        notes: Vec::new(),
    })
}

/// Blades for an axial fan by its hub ratio (more hub, more blades), as small fans are built.
fn blade_count(hub_ratio: f64) -> usize {
    (4.0 + 12.0 * (hub_ratio - 0.3)).round().clamp(4.0, 9.0) as usize
}

fn radial(flow: f64, y: f64, omega: f64, density: f64, efficiency: f64, notes: &mut Vec<String>) -> Result<Design> {
    let beta2 = 35f64.to_radians(); // outlet blade angle, back from radial
    let blades = 10usize;
    let slip = 1.0 - beta2.cos().sqrt() / (blades as f64).powf(0.7);
    let work = y / efficiency.sqrt();
    // u₂ c_u2 = work with c_u2 = σ u₂ − c_m2 tan β₂ and c_m2 = 0.25 u₂:
    // u₂² (σ − 0.25 tan β₂) = work.
    let k = slip - 0.25 * beta2.tan();
    if k <= 0.0 {
        return Err(SimError::BadDuty);
    }
    let u2 = (work / k).sqrt();
    let r2 = u2 / omega;
    let cm2 = 0.25 * u2;
    let b2 = flow / (std::f64::consts::TAU * r2 * cm2);
    // The eye: the radius that minimises the relative speed at its tip, w₁² = c₁² + (ω r₁)²
    // with c₁ = Q / (π r₁²): r₁ = (2 Q² / (π² ω²))^(1/6) · 1.1 (a little margin, the usual).
    let r1 = 1.1 * (2.0 * flow * flow / (std::f64::consts::PI.powi(2) * omega * omega)).powf(1.0 / 6.0);
    if r1 >= 0.8 * r2 {
        notes.push(format!(
            "the eye ({:.1} mm) is nearly as wide as the impeller ({:.1} mm): this duty suits an axial or mixed-flow fan",
            2e3 * r1,
            2e3 * r2
        ));
    }
    let c1 = flow / (std::f64::consts::PI * r1 * r1);
    let beta1 = (c1 / (omega * r1)).atan();
    let cu2 = work / u2;
    if b2 < 0.02 * 2.0 * r2 {
        notes.push(format!(
            "the outlet is only {:.1} mm wide ({:.1} % of the diameter): a narrow, high-pressure duty; a lower speed widens it",
            b2 * 1e3,
            100.0 * b2 / (2.0 * r2)
        ));
    }
    if b2 / (2.0 * r2) > 0.25 {
        notes.push(format!("the outlet width is {:.0} % of the diameter: a wide, low-pressure duty for a radial fan", 100.0 * b2 / (2.0 * r2)));
    }
    Ok(Design {
        kind: Kind::Radial,
        specific_speed: 0.0,
        pressure_coefficient: 2.0 * y / (u2 * u2),
        flow_coefficient: flow / (std::f64::consts::PI * r2 * r2 * u2),
        diameter: 2.0 * r2,
        inner_diameter: 2.0 * r1,
        tip_speed: u2,
        blades,
        sections: Vec::new(),
        outlet_width: b2,
        beta_outlet: beta2.to_degrees(),
        beta_inlet: beta1.to_degrees(),
        slip,
        swirl: cu2,
        work,
        torque: density * flow * r2 * cu2 / efficiency.sqrt(),
        power: density * flow * y / efficiency,
        notes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duty(flow: f64, pressure: f64, rpm: f64) -> Duty {
        Duty { flow, pressure, rpm, density: 1.204, max_diameter: None, kind: None, efficiency: 0.5 }
    }

    /// A 40 mm-class axial: 15 CFM (7.08 L/s) at 40 Pa, 6000 rpm (a printer's part-cooling fan).
    /// Axial by its specific speed; the velocity triangles close (the swirl is the work over the
    /// blade speed, the outlet angle under the inlet one at every radius); Euler's torque is the
    /// ideal power over the speed; free vortex keeps r·c_u the same at hub and tip.
    #[test]
    fn a_small_axial_fan_for_a_printer_duty() {
        let d = design(&duty(0.00708, 40.0, 6000.0)).unwrap();
        assert_eq!(d.kind, Kind::Axial, "{d:?}");
        let omega = 6000.0 * std::f64::consts::TAU / 60.0;
        assert!(d.diameter > 0.02 && d.diameter < 0.08, "{} mm", d.diameter * 1e3);
        for s in &d.sections {
            assert!(s.beta_out < s.beta_in && s.chord > 0.0 && s.solidity >= 0.4, "{s:?}");
        }
        let (hub, tip) = (d.sections.first().unwrap(), d.sections.last().unwrap());
        // r c_u2: from the triangles, c_u2 = u − c_m tan β₂.
        let cm = d.flow_coefficient * d.tip_speed;
        let rcu = |s: &Section| s.radius * (omega * s.radius - cm * s.beta_out.to_radians().tan());
        assert!((rcu(hub) / rcu(tip) - 1.0).abs() < 1e-9, "free vortex");
        assert!((rcu(tip) * omega - d.work).abs() < 1e-9 * d.work, "Euler: u c_u2 = the work");
        // Torque × speed = power on the shaft at the quoted efficiency, within its split.
        assert!((d.torque * omega / (1.204 * 0.00708 * d.work / 0.5_f64.sqrt()) - 1.0).abs() < 1e-9);
        assert!((d.power - 0.00708 * 40.0 / 0.5).abs() < 1e-12, "power is Δp Q / η");
    }

    /// A blower: 5 L/s at 400 Pa and 4000 rpm is radial by its specific speed; the slip factor is
    /// Wiesner's; the outlet carries the flow at the meridional speed chosen.
    #[test]
    fn a_blower_duty_designs_a_radial_impeller() {
        let d = design(&duty(0.005, 400.0, 4000.0)).unwrap();
        assert_eq!(d.kind, Kind::Radial, "{d:?}");
        let expect_slip = 1.0 - 35f64.to_radians().cos().sqrt() / 10f64.powf(0.7);
        assert!((d.slip - expect_slip).abs() < 1e-12);
        let r2 = d.diameter / 2.0;
        assert!((0.005 / (std::f64::consts::TAU * r2 * d.outlet_width) - 0.25 * d.tip_speed).abs() < 1e-9);
        assert!(d.inner_diameter < d.diameter);
        assert!((d.swirl * d.tip_speed - d.work).abs() < 1e-9 * d.work, "Euler");
        assert!(d.notes.iter().any(|n| n.contains("only")), "a 1.8 mm outlet on a 129 mm impeller is flagged: {:?}", d.notes);
    }

    /// The type follows the duty, a size limit asks for more speed, and bad duties are refused.
    #[test]
    fn the_duty_picks_the_type_and_limits_are_reported() {
        assert_eq!(design(&duty(0.05, 50.0, 3000.0)).unwrap().kind, Kind::Axial);
        assert_eq!(design(&duty(0.002, 1500.0, 3000.0)).unwrap().kind, Kind::Radial);
        let mut small = duty(0.00708, 40.0, 6000.0);
        small.max_diameter = Some(0.01);
        let d = design(&small).unwrap();
        assert!(d.notes.iter().any(|n| n.contains("rpm fits")), "{:?}", d.notes);
        for bad in [duty(0.0, 40.0, 6000.0), duty(0.01, -1.0, 6000.0), duty(0.01, 40.0, f64::NAN)] {
            assert!(design(&bad).is_err());
        }
    }
}

/// What a multi-stage axial compressor must do, inside a fixed tip diameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompressorDuty {
    /// Air through it, m³/s.
    pub flow: f64,
    /// Total pressure rise over all stages, Pa.
    pub pressure: f64,
    /// Air density, kg/m³.
    pub density: f64,
    /// The rotor blades' tip diameter, m.
    pub tip_diameter: f64,
    /// Hub over tip diameter, or none to pick it from the flow.
    pub hub_ratio: Option<f64>,
    /// The number of stages, or none for the fewest that keep the speed under `max_rpm` and
    /// every blade row's de Haller ratio at 0.7 or more.
    pub stages: Option<usize>,
    pub max_rpm: f64,
    /// The total efficiency assumed for the shaft power (0..1].
    pub efficiency: f64,
}

/// One radius of a stator vane: the swirl the rotor left turned back to axial.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VaneSection {
    /// Radius, m.
    pub radius: f64,
    /// Absolute flow angles in and out, degrees from the axis (out is 0: axial).
    pub alpha_in: f64,
    pub alpha_out: f64,
    pub stagger: f64,
    pub camber: f64,
    pub solidity: f64,
    /// Chord, m.
    pub chord: f64,
    /// De Haller's ratio c₃/c₂ and Lieblein's diffusion factor.
    pub de_haller: f64,
    pub diffusion: f64,
}

/// A designed compressor of repeating stages (each a rotor row and a stator row).
#[derive(Clone, Debug, PartialEq)]
pub struct Compressor {
    pub stages: usize,
    pub rpm: f64,
    /// Tip and hub diameters (m) and the blade tips' speed (m/s).
    pub tip_diameter: f64,
    pub hub_diameter: f64,
    pub tip_speed: f64,
    /// The axial speed through the annulus (m/s) and its ratio to the tip speed.
    pub axial_speed: f64,
    pub flow_coefficient: f64,
    /// Each stage's ideal work (J/kg) over the tip speed squared.
    pub stage_loading: f64,
    /// The stages' reaction at mid-span (the rotors' share of the diffusion).
    pub reaction: f64,
    pub rotor_blades: usize,
    pub stator_vanes: usize,
    /// Vanes before the first rotor (none when the stages take no pre-swirl), and in each of the
    /// two exit rows that take the last stage's swirl out.
    pub inlet_vanes: usize,
    pub exit_vanes: Vec<usize>,
    /// Every rotor row's sections hub to tip, every stator row's (between the stages), the inlet
    /// vanes' and the two exit rows'.
    pub rotor: Vec<Section>,
    pub stator: Vec<VaneSection>,
    pub inlet: Vec<VaneSection>,
    pub exit: Vec<Vec<VaneSection>>,
    /// Each stage's pressure rise, Pa, and the whole machine's shaft torque (N·m) and power (W).
    pub stage_pressure: f64,
    pub torque: f64,
    pub power: f64,
    pub notes: Vec<String>,
}

/// The least de Haller ratio a design keeps in every row, hub to tip (0.7 is where a row starts
/// to stall; the margin is for the losses the mean line leaves out).
const DE_HALLER_MIN: f64 = 0.72;

/// Design a compressor for `duty`.
pub fn compressor(duty: &CompressorDuty) -> Result<Compressor> {
    let CompressorDuty { flow, pressure, density, tip_diameter, hub_ratio, stages, max_rpm, efficiency } = *duty;
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if !(positive(flow)
        && positive(pressure)
        && positive(density)
        && positive(tip_diameter)
        && positive(max_rpm)
        && positive(efficiency)
        && efficiency <= 1.0
        && hub_ratio.is_none_or(|h| (0.2..0.95).contains(&h))
        && stages.is_none_or(|n| (1..=20).contains(&n)))
    {
        return Err(SimError::BadDuty);
    }
    let r_tip = 0.5 * tip_diameter;
    // The ideal work: the losses split evenly between the work the blades do and what reaches
    // the pressure (so torque × speed is the shaft power at the assumed efficiency).
    let work = pressure / density / efficiency.sqrt();
    let candidates: Vec<usize> = match stages {
        Some(n) => vec![n],
        None => (1..=12).collect(),
    };
    // Each stage count: search the tip speed (up to the speed limit) and the hub ratio (0.4–0.75:
    // past that the blades are so short a tip gap costs much of their span;
    // or the one given) for designs whose every row keeps de Haller's ratio; the free vortex
    // loads the hub most, so a heavier hub (faster axial flow) or a faster tip unloads it. Of
    // those, the slowest: the fewest stages that work at all win, then the lowest speed.
    let hubs: Vec<f64> = match hub_ratio {
        Some(h) => vec![h],
        None => (40..=75).map(|i| f64::from(i) / 100.0).collect(),
    };
    let max_tip = max_rpm * std::f64::consts::TAU / 60.0 * r_tip;
    let mut best_failing: Option<(f64, Compressor)> = None;
    for n in candidates {
        let mut best: Option<Compressor> = None;
        for (&h, reaction) in hubs.iter().flat_map(|h| [0.5, 0.6, 0.7, 0.8, 0.9, 1.0].map(move |r| (h, r))) {
            for i in 1..=200 {
                let tip_speed = max_tip * f64::from(i) / 200.0;
                let c = stage_design(flow, pressure, r_tip, h, tip_speed, n, work, efficiency, reaction);
                let worst = c
                    .rotor
                    .iter()
                    .map(|s| s.de_haller)
                    .chain(c.stator.iter().chain(c.exit.iter().flatten()).map(|s| s.de_haller))
                    .fold(f64::INFINITY, f64::min);
                let swirl_ok = c.rotor.iter().all(|s| s.beta_out > 0.0);
                if worst >= DE_HALLER_MIN && swirl_ok {
                    if best.as_ref().is_none_or(|b| c.rpm < b.rpm) {
                        best = Some(c);
                    }
                    break; // faster tips at this hub only spin faster
                }
                if best_failing.as_ref().is_none_or(|(w, _)| worst > *w) {
                    best_failing = Some((worst, c));
                }
            }
        }
        if let Some(c) = best {
            return Ok(c);
        }
    }
    let (worst, mut c) = best_failing.ok_or(SimError::BadDuty)?;
    c.notes.push(format!(
        "no design up to {max_rpm:.0} rpm keeps every blade row's de Haller ratio at {DE_HALLER_MIN} or more (the best reaches {worst:.2}, under 0.7 a row stalls): allow more speed or stages, or a larger diameter"
    ));
    Ok(c)
}

#[allow(clippy::too_many_arguments)]
fn stage_design(flow: f64, pressure: f64, r_tip: f64, h: f64, tip_speed: f64, n: usize, work: f64, efficiency: f64, reaction: f64) -> Compressor {
    let stage_work = work / n as f64;
    let omega = tip_speed / r_tip;
    let area = |h: f64| std::f64::consts::PI * r_tip * r_tip * (1.0 - h * h);
    let cm = flow / area(h);
    let r_hub = h * r_tip;
    let span = r_tip - r_hub;
    let r_mid = r_hub + 0.5 * span;
    // Repeating stages at `reaction` at mid-span (the rotor's share of the stage's diffusion),
    // free vortex: the swirl into each rotor and out of it, r c_u1 and r c_u2, each the same at
    // every radius, with u (c_u2 − c_u1) the stage work. At mid-span c_u1 = u (1 − R) − Δc_u/2
    // (no less than none: a lightly loaded stage gets no pre-swirl). The free vortex lowers the
    // reaction toward the hub, where the stators then diffuse the most, so a mid-span reaction
    // over a half keeps the hub's in hand. An inlet vane row gives the first rotor its swirl, every stator turns the air
    // back to the next rotor's, and the last one's swirl is taken out by exit vanes.
    let dcu_mid = stage_work / (omega * r_mid);
    let cu1_mid = (omega * r_mid * (1.0 - reaction) - 0.5 * dcu_mid).max(0.0);
    let k1 = cu1_mid * r_mid;
    let k2 = k1 + stage_work / omega;
    let vane = |r: f64, cu_in: f64, cu_out: f64| -> VaneSection {
        let (a_in, a_out) = ((cu_in / cm).atan(), (cu_out / cm).atan());
        let (c_in, c_out) = ((cm * cm + cu_in * cu_in).sqrt(), (cm * cm + cu_out * cu_out).sqrt());
        let de_haller = c_out / c_in;
        let turn = (cu_in - cu_out).abs();
        // Accelerating rows (an inlet vane) diffuse nothing: the least solidity that guides.
        let solidity = if c_out >= c_in { 0.8 } else { (turn / (2.0 * c_in * (0.45 - 1.0 + de_haller).max(0.05))).clamp(0.5, 2.0) };
        let diffusion = 1.0 - de_haller + turn / (2.0 * solidity * c_in);
        let camber = (a_in - a_out) / (1.0 - 0.26 / solidity.sqrt());
        VaneSection {
            radius: r,
            alpha_in: a_in.to_degrees(),
            alpha_out: a_out.to_degrees(),
            stagger: (0.5 * (a_in + a_out)).to_degrees(),
            camber: camber.to_degrees(),
            solidity,
            chord: 0.0,
            de_haller,
            diffusion,
        }
    };
    let mut rotor = Vec::new();
    let mut stator = Vec::new();
    let mut inlet = Vec::new();
    let mut exit = vec![Vec::new(), Vec::new()];
    for f in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let r = r_hub + f * span;
        let u = omega * r;
        let (cu1, cu2) = (k1 / r, k2 / r);
        let w1 = (cm * cm + (u - cu1).powi(2)).sqrt();
        let w2 = (cm * cm + (u - cu2).powi(2)).sqrt();
        let (beta_in, beta_out) = (((u - cu1) / cm).atan(), ((u - cu2) / cm).atan());
        let de_haller = w2 / w1;
        let dcu = cu2 - cu1;
        let solidity = (dcu / (2.0 * w1 * (0.45 - 1.0 + de_haller).max(0.05))).clamp(0.5, 2.0);
        let diffusion = 1.0 - de_haller + dcu / (2.0 * solidity * w1);
        let camber = (beta_in - beta_out) / (1.0 - 0.26 / solidity.sqrt());
        rotor.push(Section {
            radius: r,
            beta_in: beta_in.to_degrees(),
            beta_out: beta_out.to_degrees(),
            stagger: (0.5 * (beta_in + beta_out)).to_degrees(),
            camber: camber.to_degrees(),
            solidity,
            chord: 0.0,
            de_haller,
            diffusion,
        });
        stator.push(vane(r, cu2, cu1));
        if k1 > 0.0 {
            inlet.push(vane(r, 0.0, cu1));
        }
        // The last stage's swirl out in two rows, half each: only with pre-swirl (without, every
        // stator, the last one too, already turns the air back to axial).
        if k1 > 0.0
            && let [a, b] = &mut exit[..]
        {
            a.push(vane(r, cu2, 0.5 * cu2));
            b.push(vane(r, 0.5 * cu2, 0.0));
        }
    }
    // Blade counts from an aspect ratio (span over chord) of about 1.2 at mid-span, with chords of
    // at least 3.5 mm (blades that small can still be printed and hold their shape); the stator
    // gets a different count than the rotor, so their wakes do not beat in step.
    let mid = 2;
    let chord_mid = (span / 1.2).max(0.0035);
    let count = |solidity: f64| ((solidity * std::f64::consts::TAU * r_mid / chord_mid).round() as usize).clamp(5, 41);
    let rotor_blades = count(rotor.get(mid).map_or(1.0, |s| s.solidity));
    let mut stator_vanes = count(stator.get(mid).map_or(1.0, |s| s.solidity));
    if stator_vanes == rotor_blades {
        stator_vanes += 1;
    }
    let inlet_vanes = inlet.get(mid).map_or(0, |s| count(s.solidity));
    if k1 <= 0.0 {
        exit.clear();
    }
    let exit_vanes = exit.iter().map(|row| row.get(mid).map_or(0, |s| count(s.solidity))).collect::<Vec<_>>();
    for s in &mut rotor {
        s.chord = s.solidity * std::f64::consts::TAU * s.radius / rotor_blades as f64;
    }
    for s in &mut stator {
        s.chord = s.solidity * std::f64::consts::TAU * s.radius / stator_vanes as f64;
    }
    for s in &mut inlet {
        s.chord = s.solidity * std::f64::consts::TAU * s.radius / inlet_vanes.max(1) as f64;
    }
    for (row, z) in exit.iter_mut().zip(&exit_vanes) {
        for s in row.iter_mut() {
            s.chord = s.solidity * std::f64::consts::TAU * s.radius / (*z).max(1) as f64;
        }
    }
    let power = flow * pressure / efficiency;
    let mut notes = Vec::new();
    if tip_speed > 100.0 {
        notes.push(format!(
            "tip speed {tip_speed:.0} m/s (Mach {:.2}): compressibility starts to matter; the flow solver treats air as incompressible",
            tip_speed / 343.0
        ));
    }
    Compressor {
        stages: n,
        rpm: omega * 60.0 / std::f64::consts::TAU,
        tip_diameter: 2.0 * r_tip,
        hub_diameter: 2.0 * r_hub,
        tip_speed,
        axial_speed: cm,
        flow_coefficient: cm / tip_speed,
        stage_loading: stage_work / (tip_speed * tip_speed),
        reaction,
        rotor_blades,
        stator_vanes,
        inlet_vanes,
        exit_vanes,
        rotor,
        stator,
        inlet,
        exit,
        stage_pressure: pressure / n as f64,
        torque: power / omega,
        power,
        notes,
    }
}

#[cfg(test)]
mod compressor_tests {
    use super::*;

    /// The user's test case: 10 CFM at 3500 Pa through a 30 mm tip diameter. The fewest stages
    /// under 60 000 rpm with no row stalled; every row's triangles close (free vortex, Euler's
    /// work per stage), the stators turn the swirl back to axial, and torque × speed is the
    /// shaft power.
    #[test]
    fn a_30_mm_compressor_for_10_cfm_at_3500_pa() {
        let duty = CompressorDuty {
            flow: 10.0 / crate::study::CFM_PER_M3S,
            pressure: 3500.0,
            density: 1.204,
            tip_diameter: 0.030,
            hub_ratio: None,
            stages: None,
            max_rpm: 60_000.0,
            efficiency: 0.6,
        };
        let c = compressor(&duty).unwrap();
        println!("{c:#?}");
        assert!(c.stages >= 2 && c.rpm <= 60_000.0, "{} stages at {:.0} rpm", c.stages, c.rpm);
        assert!(c.rotor.iter().all(|s| s.de_haller >= 0.7) && c.stator.iter().all(|s| s.de_haller >= 0.7), "{c:#?}");
        let omega = c.rpm * std::f64::consts::TAU / 60.0;
        let stage_work = 3500.0 / 1.204 / 0.6f64.sqrt() / c.stages as f64;
        let swirl = |s: &Section, beta: f64| omega * s.radius - c.axial_speed * beta.to_radians().tan();
        for s in &c.rotor {
            let (cu1, cu2) = (swirl(s, s.beta_in), swirl(s, s.beta_out));
            assert!((omega * s.radius * (cu2 - cu1) / stage_work - 1.0).abs() < 1e-9, "Euler at r = {}", s.radius);
        }
        for (r, v) in c.rotor.iter().zip(&c.stator) {
            let (cu1, cu2) = (swirl(r, r.beta_in), swirl(r, r.beta_out));
            assert!(((cu2 / c.axial_speed).atan().to_degrees() - v.alpha_in).abs() < 1e-9, "the vane takes the swirl the rotor left");
            assert!(((cu1 / c.axial_speed).atan().to_degrees() - v.alpha_out).abs() < 1e-9, "and gives the next rotor its own");
        }
        assert!(c.exit.iter().flatten().all(|v| v.de_haller >= 0.7));
        let last = c.exit.last().unwrap_or(&c.stator);
        assert!(last.iter().all(|v| v.alpha_out.abs() < 1e-9), "the air leaves axially");
        assert!((c.torque * omega - c.power).abs() < 1e-9 * c.power);
        assert!(c.rotor_blades != c.stator_vanes);
        // One stage only: the speed goes up, and the notes say so when it passes the cap.
        let one = compressor(&CompressorDuty { stages: Some(1), ..duty }).unwrap();
        assert!(one.rpm > c.rpm && !one.notes.is_empty(), "{:?}", one.notes);
    }
}
