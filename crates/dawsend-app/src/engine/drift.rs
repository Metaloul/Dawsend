//! Régulateur de dérive d'horloge (voir ARCHITECTURE § 5.2).
//!
//! La carte du DAW (ASIO) et la sortie Windows ont chacune leur quartz : leurs fréquences
//! réelles diffèrent de quelques dizaines à quelques centaines de ppm. Sans correction, le
//! remplissage du ring dérive jusqu'à un trou (underrun) ou un retard qui grossit.
//!
//! On mesure le remplissage à chaque callback, on le lisse (il forme une dent de scie de
//! l'amplitude d'un bloc du DAW), et un régulateur PI en déduit une petite correction du
//! rapport de rééchantillonnage, bornée à ± 1 000 ppm. La variation de hauteur
//! correspondante (au plus 1,7 cent) est inaudible.
//!
//! Tout est en secondes d'audio source, donc indépendant des fréquences. Temps réel :
//! aucune allocation.

/// Constante de temps du lissage du remplissage.
const SMOOTHING_S: f64 = 1.0;
/// Gain proportionnel : 1 ms d'écart → 100 ppm de correction.
const KP: f64 = 0.1;
/// Gain intégral : amortissement critique avec `KP` (ζ = KP / (2·√KI) = 1).
const KI: f64 = 0.0025;
/// Correction maximale.
pub const MAX_CORRECTION: f64 = 1_000e-6;

/// Fenêtre de mesure de la profondeur de la dent de scie, et nombre de fenêtres gardées
/// (4 s d'historique).
const WINDOW_S: f64 = 0.5;
const WINDOWS: usize = 8;
/// Erreur maximale prise en compte par l'intégrale (l'estimation de la dérive) : un pic du
/// DAW, une resynchro ou un changement de marge la font à peine bouger, alors qu'une vraie
/// dérive, qui dure, finit toujours par être intégrée.
const INTEGRATE_CLAMP_S: f64 = 0.002;

#[derive(Debug, Clone)]
pub struct DriftController {
    smoothed: f64,
    integral: f64,
    correction: f64,
    initialized: bool,
    /// Creux maximal (remplissage lissé − remplissage instantané) de chaque fenêtre
    /// terminée, en secondes.
    history: [f64; WINDOWS],
    next: usize,
    windows_seen: usize,
    /// Creux maximal de la fenêtre en cours, et sa durée écoulée.
    window_max: f64,
    window_elapsed: f64,
    /// Profondeur retenue de la dent de scie, en secondes (voir [`Self::spread`]).
    spread: f64,
}

impl Default for DriftController {
    fn default() -> Self {
        Self::new()
    }
}

impl DriftController {
    pub fn new() -> Self {
        Self {
            smoothed: 0.0,
            integral: 0.0,
            correction: 0.0,
            initialized: false,
            history: [0.0; WINDOWS],
            next: 0,
            windows_seen: 0,
            window_max: 0.0,
            window_elapsed: 0.0,
            spread: 0.0,
        }
    }

    /// Repart d'un remplissage connu (après un amorçage ou une resynchro). L'intégrale
    /// (l'estimation de la dérive) et l'historique des creux sont conservés : ils restent
    /// valables.
    pub fn restart(&mut self) {
        self.initialized = false;
    }

    /// Signale qu'il a manqué `seconds` de données (underrun alors que le DAW tourne) :
    /// la dent de scie est au moins aussi profonde que la marge actuelle plus ce manque.
    pub fn note_shortfall(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.window_max = self.window_max.max(self.spread + seconds);
            self.spread = self.robust_spread();
        }
    }

    /// Profondeur retenue : pendant les premières secondes, le plus grand creux vu (la
    /// marge s'adapte tout de suite aux gros blocs du DAW) ; ensuite, le deuxième plus
    /// grand creux des 4 dernières secondes. Les gros blocs creusent la dent de scie dans
    /// chaque fenêtre et sont donc pris en compte ; un pic isolé du DAW (une seule
    /// fenêtre) ne relève pas la marge, donc pas la latence.
    fn robust_spread(&self) -> f64 {
        let filled = self.windows_seen.min(WINDOWS);
        let mut first = self.window_max;
        let mut second = 0.0f64;
        for &v in &self.history[..filled] {
            if v > first {
                second = first;
                first = v;
            } else if v > second {
                second = v;
            }
        }
        if self.windows_seen < WINDOWS {
            first
        } else {
            second
        }
    }

    /// Correction relative actuelle : > 0 → il faut consommer la source plus vite.
    pub fn correction(&self) -> f64 {
        self.correction
    }

    /// Remplissage lissé, en secondes.
    pub fn smoothed(&self) -> f64 {
        self.smoothed
    }

    /// Profondeur mesurée de la dent de scie du remplissage, en secondes.
    pub fn spread(&self) -> f64 {
        self.spread
    }

    /// Nouvelle mesure du remplissage `fill` (s), pour la cible `target` (s), `dt` (s)
    /// après la précédente. Renvoie la correction relative à appliquer.
    pub fn update(&mut self, fill: f64, target: f64, dt: f64) -> f64 {
        if !dt.is_finite() || dt <= 0.0 || !fill.is_finite() {
            return self.correction;
        }
        if !self.initialized {
            self.smoothed = fill;
            self.initialized = true;
        } else {
            let a = 1.0 - (-dt / SMOOTHING_S).exp();
            self.smoothed += (fill - self.smoothed) * a;
        }
        let dip = (self.smoothed - fill).max(0.0);
        self.window_max = self.window_max.max(dip);
        self.window_elapsed += dt;
        if self.window_elapsed >= WINDOW_S {
            self.history[self.next] = self.window_max;
            self.next = (self.next + 1) % WINDOWS;
            self.windows_seen = self.windows_seen.saturating_add(1);
            self.window_max = 0.0;
            self.window_elapsed = 0.0;
        }
        self.spread = self.robust_spread();

        let err = self.smoothed - target;
        // Anti-emballement : l'intégrale seule ne peut pas dépasser la correction max.
        let max_integral = MAX_CORRECTION / KI;
        let e = err.clamp(-INTEGRATE_CLAMP_S, INTEGRATE_CLAMP_S);
        self.integral = (self.integral + e * dt).clamp(-max_integral, max_integral);
        self.correction = (KP * err + KI * self.integral).clamp(-MAX_CORRECTION, MAX_CORRECTION);
        self.correction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simule un ring alimenté avec une horloge décalée de `drift_ppm` par rapport à la
    /// sortie, et renvoie (écart final au cible en ms, correction finale en ppm).
    fn simulate(drift_ppm: f64, seconds: f64) -> (f64, f64) {
        let mut c = DriftController::new();
        let dt = 0.01;
        let target = 0.010;
        let mut fill = target;
        let mut worst_late = 0.0f64;
        let steps = (seconds / dt) as usize;
        for i in 0..steps {
            // Entrée : 10 ms × (1 + dérive) ; sortie : 10 ms × (1 + correction).
            fill += dt * (1.0 + drift_ppm * 1e-6) - dt * (1.0 + c.correction());
            let corr = c.update(fill, target, dt);
            assert!(corr.abs() <= MAX_CORRECTION);
            if i > steps / 2 {
                worst_late = worst_late.max((fill - target).abs());
            }
        }
        (worst_late * 1000.0, c.correction() * 1e6)
    }

    #[test]
    fn compensates_typical_drift() {
        for drift in [-300.0, -50.0, 0.0, 80.0, 200.0, 500.0] {
            let (late_ms, ppm) = simulate(drift, 600.0);
            assert!(late_ms < 0.2, "dérive {drift} ppm : écart {late_ms} ms");
            assert!(
                (ppm - drift).abs() < 5.0,
                "dérive {drift} : correction {ppm} ppm"
            );
        }
    }

    #[test]
    fn correction_is_bounded() {
        // Dérive hors limites : la correction plafonne à 1 000 ppm, sans emballement.
        let (_, ppm) = simulate(5_000.0, 60.0);
        assert!((ppm - 1_000.0).abs() < 1e-6);
    }

    #[test]
    fn spread_tracks_sawtooth() {
        let mut c = DriftController::new();
        // Dent de scie : un bloc de 40 ms toutes les 4 mesures de 10 ms.
        for i in 0..2_000 {
            let fill = 0.010 + 0.040 - 0.010 * f64::from(i % 4);
            c.update(fill, 0.010, 0.01);
        }
        // Moyenne 35 ms, minimum 20 ms : la dent de scie descend de 15 ms sous la moyenne.
        assert!(c.spread() > 0.013 && c.spread() < 0.017, "{}", c.spread());
    }

    /// Remplissage stable à 10 ms, avec un creux de `depth` s pendant 20 ms aux instants
    /// (en s) de `spikes`. Renvoie la profondeur retenue à la fin.
    fn spread_after(spikes: &[f64], depth: f64, seconds: f64) -> f64 {
        let mut c = DriftController::new();
        let dt = 0.01;
        for i in 0..(seconds / dt) as usize {
            let t = i as f64 * dt;
            let spiking = spikes.iter().any(|&s| t >= s && t < s + 0.02);
            let fill = if spiking { 0.010 - depth } else { 0.010 };
            c.update(fill, 0.010, dt);
        }
        c.spread()
    }

    #[test]
    fn isolated_spike_does_not_raise_the_margin() {
        // Un pic de 8 ms après la phase d'apprentissage : ignoré tout de suite.
        assert!(spread_after(&[6.0], 0.008, 6.5) < 0.001);
        // Deux pics en moins de 4 s : la marge monte.
        assert!(spread_after(&[6.0, 7.2], 0.008, 7.5) > 0.006);
        // Les pics anciens sortent de l'historique.
        assert!(spread_after(&[6.0, 7.2], 0.008, 12.0) < 0.001);
    }

    #[test]
    fn spike_does_not_corrupt_the_drift_estimate() {
        // Dérive de 200 ppm compensée, puis le remplissage chute de 8 ms d'un coup (DAW
        // qui a perdu un bloc) : l'intégrale ne doit pas s'emballer.
        let mut c = DriftController::new();
        let dt = 0.01;
        let target = 0.010;
        let mut fill = target;
        for i in 0..60_000 {
            if i == 30_000 {
                fill -= 0.008;
            }
            fill += dt * 200e-6 - dt * c.correction();
            c.update(fill, target, dt);
        }
        assert!((c.correction() * 1e6 - 200.0).abs() < 5.0);
        assert!((fill - target).abs() < 0.0005);
    }
}
