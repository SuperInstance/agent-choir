//! # agent-choir
//!
//! Scaling from quartet (4 agents) to choir (100+ agents).
//!
//! At quartet scale, every agent hears every other. At choir scale, you need
//! sections — groups of agents that share a role, a range, and a leader.
//! This crate provides the primitives for organising, scoring, and directing
//! large ensembles of cooperative agents.

use std::collections::HashMap;

// ── Voice section ──────────────────────────────────────────────────────────

/// The four classical voice parts, ordered from highest to lowest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoicePart {
    Soprano,
    Alto,
    Tenor,
    Bass,
}

impl VoicePart {
    /// Return a rough default pitch-range floor (MIDI note number).
    pub fn default_floor(&self) -> u8 {
        match self {
            VoicePart::Soprano => 60,
            VoicePart::Alto => 55,
            VoicePart::Tenor => 48,
            VoicePart::Bass => 40,
        }
    }

    /// Return a rough default pitch-range ceiling (MIDI note number).
    pub fn default_ceiling(&self) -> u8 {
        match self {
            VoicePart::Soprano => 81,
            VoicePart::Alto => 74,
            VoicePart::Tenor => 67,
            VoicePart::Bass => 60,
        }
    }

    /// All four parts in score order.
    pub fn all() -> &'static [VoicePart] {
        &[
            VoicePart::Soprano,
            VoicePart::Alto,
            VoicePart::Tenor,
            VoicePart::Bass,
        ]
    }
}

/// An individual singer/agent in the choir.
#[derive(Debug, Clone)]
pub struct Singer {
    pub id: String,
    pub preferred_part: VoicePart,
    /// 0.0 – 1.0: how versatile is this singer at covering other parts?
    pub versatility: f64,
    /// 0.0 – 1.0: base skill level.
    pub skill: f64,
}

/// A voice section groups N agents under one part.
#[derive(Debug, Clone)]
pub struct VoiceSection {
    pub part: VoicePart,
    pub singers: Vec<Singer>,
}

impl VoiceSection {
    pub fn new(part: VoicePart) -> Self {
        Self {
            part,
            singers: Vec::new(),
        }
    }

    pub fn add(&mut self, singer: Singer) {
        self.singers.push(singer);
    }

    pub fn len(&self) -> usize {
        self.singers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.singers.is_empty()
    }

    /// Average skill of singers in this section.
    pub fn avg_skill(&self) -> f64 {
        if self.singers.is_empty() {
            return 0.0;
        }
        self.singers.iter().map(|s| s.skill).sum::<f64>() / self.singers.len() as f64
    }
}

// ── Blend score ────────────────────────────────────────────────────────────

/// How well agents within a section blend with each other.
///
/// A section blends well when its members have similar skill levels and
/// compatible preferred parts. The score is 0.0 – 1.0.
#[derive(Debug, Clone)]
pub struct BlendScore {
    pub section: VoicePart,
    pub score: f64,
    pub breakdown: BlendBreakdown,
}

#[derive(Debug, Clone)]
pub struct BlendBreakdown {
    /// How similar the skill levels are (1.0 = identical).
    pub skill_uniformity: f64,
    /// Fraction of singers who prefer *this* part.
    pub part_affinity: f64,
    /// Overall versatility helps with blending.
    pub avg_versatility: f64,
}

/// Compute the blend score for a voice section.
pub fn compute_blend(section: &VoiceSection) -> BlendScore {
    if section.singers.is_empty() {
        return BlendScore {
            section: section.part,
            score: 0.0,
            breakdown: BlendBreakdown {
                skill_uniformity: 0.0,
                part_affinity: 0.0,
                avg_versatility: 0.0,
            },
        };
    }

    let skills: Vec<f64> = section.singers.iter().map(|s| s.skill).collect();
    let mean = skills.iter().sum::<f64>() / skills.len() as f64;
    let variance = skills.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / skills.len() as f64;
    // Uniformity: low variance → high uniformity.  Cap at 1.0.
    let skill_uniformity = (1.0 - variance).max(0.0).min(1.0);

    let matching = section
        .singers
        .iter()
        .filter(|s| s.preferred_part == section.part)
        .count();
    let part_affinity = matching as f64 / section.singers.len() as f64;

    let avg_versatility = section.singers.iter().map(|s| s.versatility).sum::<f64>()
        / section.singers.len() as f64;

    // Weighted blend of the three factors.
    let score = (skill_uniformity * 0.4 + part_affinity * 0.4 + avg_versatility * 0.2).min(1.0);

    BlendScore {
        section: section.part,
        score,
        breakdown: BlendBreakdown {
            skill_uniformity,
            part_affinity,
            avg_versatility,
        },
    }
}

// ── Choir balance ──────────────────────────────────────────────────────────

/// Section-to-section balance assessment.
///
/// A choir is balanced when sections have comparable size and skill.
#[derive(Debug, Clone)]
pub struct ChoirBalance {
    pub section_sizes: HashMap<VoicePart, usize>,
    pub section_avg_skills: HashMap<VoicePart, f64>,
    pub size_balance: f64,
    pub skill_balance: f64,
    pub overall: f64,
}

/// Compute balance across a set of voice sections.
pub fn compute_balance(sections: &[VoiceSection]) -> ChoirBalance {
    let mut section_sizes: HashMap<VoicePart, usize> = HashMap::new();
    let mut section_avg_skills: HashMap<VoicePart, f64> = HashMap::new();

    for sec in sections {
        section_sizes.insert(sec.part, sec.len());
        section_avg_skills.insert(sec.part, sec.avg_skill());
    }

    let sizes: Vec<f64> = section_sizes.values().map(|&s| s as f64).collect();
    let size_balance = balance_coefficient(&sizes);

    let skills: Vec<f64> = section_avg_skills.values().cloned().collect();
    let skill_balance = balance_coefficient(&skills);

    let overall = (size_balance * 0.5 + skill_balance * 0.5).min(1.0);

    ChoirBalance {
        section_sizes,
        section_avg_skills,
        size_balance,
        skill_balance,
        overall,
    }
}

/// Simple balance coefficient: 1.0 when all values are equal, decreasing
/// toward 0.0 as they diverge.  Uses coefficient of variation.
fn balance_coefficient(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 1.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if mean.abs() < f64::EPSILON {
        return 1.0;
    }
    let std_dev = {
        let v = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;
        v.sqrt()
    };
    let cv = std_dev / mean;
    // cv of 0 → perfectly balanced (1.0); cv >= 1.0 → terrible (0.0).
    (1.0 - cv).max(0.0).min(1.0)
}

// ── Score allocation ───────────────────────────────────────────────────────

/// Decision about which agent sings which part.
#[derive(Debug, Clone)]
pub struct Allocation {
    pub singer_id: String,
    pub assigned_part: VoicePart,
    pub confidence: f64,
}

/// Assign singers to parts based on their capabilities.
///
/// Greedy algorithm: each singer is assigned to their preferred part first;
/// if a section is overfull, the lowest-skill extras are redistributed to
/// the section where they'd be most useful (based on versatility).
pub fn allocate_parts(
    singers: &[Singer],
    target_sizes: &HashMap<VoicePart, usize>,
) -> Vec<Allocation> {
    let mut assignments: HashMap<VoicePart, Vec<&Singer>> = HashMap::new();
    for part in VoicePart::all() {
        assignments.entry(*part).or_default();
    }

    // First pass: assign everyone to preferred part.
    for singer in singers {
        assignments
            .entry(singer.preferred_part)
            .or_default()
            .push(singer);
    }

    // Redistribute overflow.
    for part in VoicePart::all() {
        let target = target_sizes.get(part).copied().unwrap_or(0);
        let assigned = assignments.get_mut(part).unwrap();
        if assigned.len() > target {
            // Sort by skill descending — keep the best, overflow the weakest.
            assigned.sort_by(|a, b| b.skill.partial_cmp(&a.skill).unwrap());
            let overflow: Vec<&Singer> = assigned.drain(target..).collect();
            for singer in overflow {
                // Find the section most in need.
                let best_part = VoicePart::all()
                    .iter()
                    .filter(|p| *p != part)
                    .min_by_key(|p| {
                        let current = assignments.get(p).map(|v| v.len()).unwrap_or(0);
                        let target_p = target_sizes.get(p).copied().unwrap_or(0);
                        // Negative = underfull; most negative = most need.
                        (current as i64 - target_p as i64)
                    })
                    .copied()
                    .unwrap_or(singer.preferred_part);
                assignments.get_mut(&best_part).unwrap().push(singer);
            }
        }
    }

    // Build allocation results.
    let mut results = Vec::new();
    for (part, assigned) in &assignments {
        for singer in assigned {
            let confidence = if singer.preferred_part == *part {
                singer.skill
            } else {
                singer.skill * singer.versatility
            };
            results.push(Allocation {
                singer_id: singer.id.clone(),
                assigned_part: *part,
                confidence: confidence.min(1.0),
            });
        }
    }
    results
}

// ── Choir director ─────────────────────────────────────────────────────────

/// A directive the director issues to a section.
#[derive(Debug, Clone)]
pub enum Directive {
    /// Grow the section (recruit more agents).
    Grow { target: usize },
    /// Shrink the section (release agents).
    Shrink { target: usize },
    /// Rehearse — improve blend score.
    Rehearse,
    /// Hold — section is fine as-is.
    Hold,
}

/// The choir director coordinates sections without micromanaging.
#[derive(Debug, Clone)]
pub struct ChoirDirector {
    pub sections: HashMap<VoicePart, VoiceSection>,
    pub target_sizes: HashMap<VoicePart, usize>,
    pub blend_threshold: f64,
    pub balance_threshold: f64,
}

impl ChoirDirector {
    pub fn new(
        target_sizes: HashMap<VoicePart, usize>,
        blend_threshold: f64,
        balance_threshold: f64,
    ) -> Self {
        let mut sections = HashMap::new();
        for part in VoicePart::all() {
            sections.insert(*part, VoiceSection::new(*part));
        }
        Self {
            sections,
            target_sizes,
            blend_threshold,
            balance_threshold,
        }
    }

    /// Add a singer to the section matching their preferred part.
    pub fn add_singer(&mut self, singer: Singer) {
        let part = singer.preferred_part;
        self.sections.get_mut(&part).unwrap().add(singer);
    }

    /// Assess the choir and return directives for each section.
    pub fn assess(&self) -> HashMap<VoicePart, Directive> {
        let mut directives = HashMap::new();

        for part in VoicePart::all() {
            let section = self.sections.get(part).unwrap();
            let target = self.target_sizes.get(part).copied().unwrap_or(0);

            let directive = if section.len() < target {
                Directive::Grow {
                    target: target - section.len(),
                }
            } else if section.len() > target {
                Directive::Shrink {
                    target: section.len() - target,
                }
            } else {
                let blend = compute_blend(section);
                if blend.score < self.blend_threshold {
                    Directive::Rehearse
                } else {
                    Directive::Hold
                }
            };
            directives.insert(*part, directive);
        }

        directives
    }

    /// Run a full assessment: balance + per-section blend + directives.
    pub fn full_assessment(&self) -> ChoirAssessment {
        let section_vec: Vec<VoiceSection> = self.sections.values().cloned().collect();
        let balance = compute_balance(&section_vec);
        let blends: HashMap<VoicePart, BlendScore> = self
            .sections
            .iter()
            .map(|(part, sec)| (*part, compute_blend(sec)))
            .collect();
        let directives = self.assess();

        ChoirAssessment {
            balance,
            blends,
            directives,
        }
    }
}

/// Full choir assessment result.
#[derive(Debug, Clone)]
pub struct ChoirAssessment {
    pub balance: ChoirBalance,
    pub blends: HashMap<VoicePart, BlendScore>,
    pub directives: HashMap<VoicePart, Directive>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_singer(id: &str, part: VoicePart, skill: f64, versatility: f64) -> Singer {
        Singer {
            id: id.to_string(),
            preferred_part: part,
            skill,
            versatility,
        }
    }

    // ── VoicePart tests ────────────────────────────────────────────────

    #[test]
    fn voice_part_ordering() {
        assert!(VoicePart::Soprano.default_floor() > VoicePart::Alto.default_floor());
        assert!(VoicePart::Alto.default_floor() > VoicePart::Tenor.default_floor());
        assert!(VoicePart::Tenor.default_floor() > VoicePart::Bass.default_floor());
    }

    #[test]
    fn voice_part_ranges_valid() {
        for part in VoicePart::all() {
            assert!(part.default_floor() < part.default_ceiling());
        }
    }

    #[test]
    fn voice_part_all_has_four() {
        assert_eq!(VoicePart::all().len(), 4);
    }

    // ── VoiceSection tests ─────────────────────────────────────────────

    #[test]
    fn section_creation() {
        let sec = VoiceSection::new(VoicePart::Soprano);
        assert_eq!(sec.part, VoicePart::Soprano);
        assert!(sec.is_empty());
        assert_eq!(sec.len(), 0);
        assert_eq!(sec.avg_skill(), 0.0);
    }

    #[test]
    fn section_add_singers() {
        let mut sec = VoiceSection::new(VoicePart::Tenor);
        sec.add(make_singer("t1", VoicePart::Tenor, 0.8, 0.5));
        sec.add(make_singer("t2", VoicePart::Tenor, 0.6, 0.3));
        assert_eq!(sec.len(), 2);
        assert!((sec.avg_skill() - 0.7).abs() < 1e-9);
    }

    // ── Blend score tests ──────────────────────────────────────────────

    #[test]
    fn blend_empty_section() {
        let sec = VoiceSection::new(VoicePart::Bass);
        let blend = compute_blend(&sec);
        assert_eq!(blend.score, 0.0);
    }

    #[test]
    fn blend_perfect_section() {
        let mut sec = VoiceSection::new(VoicePart::Soprano);
        for i in 0..5 {
            sec.add(make_singer(&format!("s{i}"), VoicePart::Soprano, 0.9, 1.0));
        }
        let blend = compute_blend(&sec);
        // All same skill, all same preferred part, high versatility → near 1.0.
        assert!(blend.score > 0.9);
        assert_eq!(blend.breakdown.skill_uniformity, 1.0);
        assert_eq!(blend.breakdown.part_affinity, 1.0);
    }

    #[test]
    fn blend_mixed_section() {
        let mut sec = VoiceSection::new(VoicePart::Alto);
        sec.add(make_singer("a1", VoicePart::Alto, 0.9, 0.5));
        sec.add(make_singer("a2", VoicePart::Soprano, 0.3, 0.2)); // misplaced
        let blend = compute_blend(&sec);
        assert!(blend.breakdown.part_affinity < 1.0);
        assert!(blend.breakdown.skill_uniformity < 1.0);
        assert!(blend.score < 0.9);
    }

    // ── Balance tests ──────────────────────────────────────────────────

    #[test]
    fn balance_perfect() {
        let sections: Vec<VoiceSection> = VoicePart::all()
            .iter()
            .map(|part| {
                let mut sec = VoiceSection::new(*part);
                for i in 0..10 {
                    sec.add(make_singer(
                        &format!("{:?}{i}", part),
                        *part,
                        0.8,
                        0.5,
                    ));
                }
                sec
            })
            .collect();

        let bal = compute_balance(&sections);
        assert!(bal.size_balance > 0.99);
        assert!(bal.skill_balance > 0.99);
        assert!(bal.overall > 0.99);
    }

    #[test]
    fn balance_unequal_sizes() {
        let mut s = VoiceSection::new(VoicePart::Soprano);
        for i in 0..20 {
            s.add(make_singer(&format!("s{i}"), VoicePart::Soprano, 0.8, 0.5));
        }
        let mut a = VoiceSection::new(VoicePart::Alto);
        a.add(make_singer("a1", VoicePart::Alto, 0.8, 0.5));
        let bal = compute_balance(&[s, a]);
        assert!(bal.size_balance < 0.5);
    }

    #[test]
    fn balance_empty_sections() {
        let bal = compute_balance(&[]);
        assert_eq!(bal.overall, 1.0);
    }

    // ── Allocation tests ───────────────────────────────────────────────

    #[test]
    fn allocation_preferred_parts() {
        let singers = vec![
            make_singer("s1", VoicePart::Soprano, 0.9, 0.5),
            make_singer("a1", VoicePart::Alto, 0.8, 0.5),
        ];
        let mut targets = HashMap::new();
        targets.insert(VoicePart::Soprano, 1);
        targets.insert(VoicePart::Alto, 1);
        targets.insert(VoicePart::Tenor, 0);
        targets.insert(VoicePart::Bass, 0);

        let allocs = allocate_parts(&singers, &targets);
        assert_eq!(allocs.len(), 2);
        let s_alloc = allocs.iter().find(|a| a.singer_id == "s1").unwrap();
        assert_eq!(s_alloc.assigned_part, VoicePart::Soprano);
        let a_alloc = allocs.iter().find(|a| a.singer_id == "a1").unwrap();
        assert_eq!(a_alloc.assigned_part, VoicePart::Alto);
    }

    #[test]
    fn allocation_overflow_redistributes() {
        // 4 sopranos but only 1 slot → 3 overflow.
        let singers = vec![
            make_singer("s1", VoicePart::Soprano, 0.9, 0.8),
            make_singer("s2", VoicePart::Soprano, 0.7, 0.9),
            make_singer("s3", VoicePart::Soprano, 0.5, 0.6),
            make_singer("s4", VoicePart::Soprano, 0.3, 0.4),
        ];
        let mut targets = HashMap::new();
        targets.insert(VoicePart::Soprano, 1);
        targets.insert(VoicePart::Alto, 1);
        targets.insert(VoicePart::Tenor, 1);
        targets.insert(VoicePart::Bass, 1);

        let allocs = allocate_parts(&singers, &targets);
        // Best singer stays in soprano.
        let s_alloc = allocs.iter().find(|a| a.singer_id == "s1").unwrap();
        assert_eq!(s_alloc.assigned_part, VoicePart::Soprano);
        // The other 3 should be distributed to other parts.
        let parts: std::collections::HashSet<VoicePart> =
            allocs.iter().map(|a| a.assigned_part).collect();
        assert!(parts.contains(&VoicePart::Alto) || parts.contains(&VoicePart::Tenor) || parts.contains(&VoicePart::Bass));
    }

    // ── Director tests ─────────────────────────────────────────────────

    #[test]
    fn director_hold_when_balanced() {
        let mut targets = HashMap::new();
        targets.insert(VoicePart::Soprano, 2);
        targets.insert(VoicePart::Alto, 2);
        targets.insert(VoicePart::Tenor, 2);
        targets.insert(VoicePart::Bass, 2);

        let mut director = ChoirDirector::new(targets, 0.5, 0.5);
        for i in 0..8 {
            let part = VoicePart::all()[i % 4];
            director.add_singer(make_singer(&format!("singer{i}"), part, 0.8, 0.7));
        }

        let directives = director.assess();
        for part in VoicePart::all() {
            match directives.get(part) {
                Some(Directive::Hold) | Some(Directive::Rehearse) => {} // acceptable
                Some(Directive::Grow { .. }) | Some(Directive::Shrink { .. }) => {
                    // With exactly target sizes, should not need grow/shrink.
                    panic!("{part:?} should be Hold or Rehearse, got {:?}", directives.get(part));
                }
                None => panic!("Missing directive for {part:?}"),
            }
        }
    }

    #[test]
    fn director_grow_when_understaffed() {
        let mut targets = HashMap::new();
        targets.insert(VoicePart::Soprano, 10);
        targets.insert(VoicePart::Alto, 10);
        targets.insert(VoicePart::Tenor, 10);
        targets.insert(VoicePart::Bass, 10);

        let mut director = ChoirDirector::new(targets, 0.5, 0.5);
        director.add_singer(make_singer("s1", VoicePart::Soprano, 0.9, 0.5));

        let directives = director.assess();
        match directives.get(&VoicePart::Soprano).unwrap() {
            Directive::Grow { target } => assert!(*target > 0),
            other => panic!("Expected Grow, got {other:?}"),
        }
    }

    #[test]
    fn director_full_assessment() {
        let mut targets = HashMap::new();
        targets.insert(VoicePart::Soprano, 3);
        targets.insert(VoicePart::Alto, 3);
        targets.insert(VoicePart::Tenor, 3);
        targets.insert(VoicePart::Bass, 3);

        let mut director = ChoirDirector::new(targets, 0.3, 0.3);
        for i in 0..12 {
            let part = VoicePart::all()[i % 4];
            director.add_singer(make_singer(&format!("s{i}"), part, 0.7 + (i as f64 * 0.02), 0.6));
        }

        let assessment = director.full_assessment();
        assert!(assessment.balance.overall > 0.0);
        assert_eq!(assessment.blends.len(), 4);
        assert_eq!(assessment.directives.len(), 4);
    }

    #[test]
    fn director_shrink_when_overstaffed() {
        let mut targets = HashMap::new();
        targets.insert(VoicePart::Soprano, 1);
        targets.insert(VoicePart::Alto, 1);
        targets.insert(VoicePart::Tenor, 1);
        targets.insert(VoicePart::Bass, 1);

        let mut director = ChoirDirector::new(targets, 0.5, 0.5);
        // Add 5 sopranos when target is 1.
        for i in 0..5 {
            director.add_singer(make_singer(&format!("s{i}"), VoicePart::Soprano, 0.7, 0.5));
        }
        // Add 1 to other sections.
        director.add_singer(make_singer("a1", VoicePart::Alto, 0.7, 0.5));
        director.add_singer(make_singer("t1", VoicePart::Tenor, 0.7, 0.5));
        director.add_singer(make_singer("b1", VoicePart::Bass, 0.7, 0.5));

        let directives = director.assess();
        match directives.get(&VoicePart::Soprano).unwrap() {
            Directive::Shrink { target } => assert_eq!(*target, 4),
            other => panic!("Expected Shrink, got {other:?}"),
        }
    }

    // ── Scale test ─────────────────────────────────────────────────────

    #[test]
    fn choir_of_100() {
        let mut targets = HashMap::new();
        targets.insert(VoicePart::Soprano, 30);
        targets.insert(VoicePart::Alto, 25);
        targets.insert(VoicePart::Tenor, 25);
        targets.insert(VoicePart::Bass, 20);

        let mut director = ChoirDirector::new(targets, 0.3, 0.3);
        for i in 0..100 {
            let part = VoicePart::all()[i % 4];
            director.add_singer(make_singer(
                &format!("singer{i:03}"),
                part,
                0.5 + (i as f64 % 50.0) / 100.0,
                0.3 + (i as f64 % 30.0) / 100.0,
            ));
        }

        let assessment = director.full_assessment();
        assert!(assessment.balance.overall > 0.0);
        // All sections should have been created and assessed.
        assert_eq!(assessment.blends.len(), 4);
    }
}
