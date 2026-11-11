//! Preencher > Série (Início > Preenchimento > Série...): o motor das séries
//! do Excel que a alça de preenchimento (`fill.rs`) não cobre. Linear e
//! crescimento (geométrica) com incremento e limite, datas por dia, dia útil,
//! mês e ano, e tendência linear/exponencial por mínimos quadrados.
//!
//! A origem (`origem`) são as sementes; o destino (`destino`) é o retângulo
//! inteiro a preencher, que começa na origem e a contém (como na caixa
//! "Série" do Excel, em que a seleção inclui as sementes). Só avança para
//! baixo (série em colunas) ou para a direita (série em linhas). Sem
//! `incremento`, a série continua depois da última semente, que fica como
//! está; com `incremento`, parte da primeira semente (como o Excel) e
//! reescreve as sementes. Tendência mantém as sementes e ignora o incremento.

use super::fill::{format_series, FillCell, SourceCell};
use crate::celulas::date_serial::serial_to_ymd;
use crate::celulas::input::date_to_serial;
use crate::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};
use crate::celulas::values::Value;
use crate::planilha::sheet::Sheet;

/// Teto de células por linha quando só o limite manda (destino = origem).
const TETO_SEM_DESTINO: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnidadeData {
    Dia,
    /// Dias úteis: pula sábado e domingo.
    DiaUtil,
    Mes,
    Ano,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoSerie {
    /// Soma o incremento a cada passo.
    Linear,
    /// Crescimento: multiplica pelo incremento a cada passo.
    Geometrica,
    Data(UnidadeData),
    /// Reta de mínimos quadrados sobre as sementes (x = 1, 2, ...).
    TendenciaLinear,
    /// Curva exponencial y = b·m^x ajustada sobre as sementes (sementes > 0).
    TendenciaExponencial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirecaoSerie {
    /// Cada coluna vira uma série que desce.
    Colunas,
    /// Cada linha vira uma série que vai para a direita.
    Linhas,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfigSerie {
    pub tipo: TipoSerie,
    pub direcao: DirecaoSerie,
    /// "Incremento" da caixa. `None` = deduzir das sementes (padrão 1).
    pub incremento: Option<f64>,
    /// "Parar em": a série não passa desse valor.
    pub limite: Option<f64>,
}

impl ConfigSerie {
    pub fn nova(tipo: TipoSerie, direcao: DirecaoSerie) -> Self {
        Self { tipo, direcao, incremento: None, limite: None }
    }
}

fn dia_da_semana(serial: i64) -> i64 {
    // 0 = domingo ... 6 = sábado (serial 45000 = quarta, 15/03/2023).
    (serial + 6).rem_euclid(7)
}

fn fim_de_semana(serial: i64) -> bool {
    matches!(dia_da_semana(serial), 0 | 6)
}

fn soma_uteis(mut s: i64, passos: i64) -> i64 {
    let d = passos.signum();
    for _ in 0..passos.abs() {
        s += d;
        while fim_de_semana(s) {
            s += d;
        }
    }
    s
}

fn soma_meses(inicio: i64, meses: i64) -> i64 {
    let (a, m, d) = serial_to_ymd(inicio);
    let total = i64::from(a) * 12 + i64::from(m) - 1 + meses;
    let ano = total.div_euclid(12) as i32;
    let mes = total.rem_euclid(12) as i32 + 1;
    let dias_no_mes = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (ano % 4 == 0 && ano % 100 != 0) || ano % 400 == 0 { 29 } else { 28 }
        }
    };
    date_to_serial(ano, mes, (d as i32).min(dias_no_mes)) as i64
}

/// Mínimos quadrados de y sobre x = 1..n: (inclinação, intercepto).
fn minimos_quadrados(y: &[f64]) -> (f64, f64) {
    let n = y.len() as f64;
    let mx = (n + 1.0) / 2.0;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (i, v) in y.iter().enumerate() {
        let dx = (i + 1) as f64 - mx;
        sxy += dx * (v - my);
        sxx += dx * dx;
    }
    let b = if sxx == 0.0 { 0.0 } else { sxy / sxx };
    (b, my - b * mx)
}

/// Gerador dos valores de uma linha de série. `k` é a posição contada da
/// primeira semente (0). Devolve o valor da posição `k` ou erro.
struct Gerador {
    sementes: Vec<f64>,
    cfg: ConfigSerie,
    passo: f64,
    reta: (f64, f64),
}

impl Gerador {
    fn novo(sementes: Vec<f64>, cfg: ConfigSerie) -> Result<Self, String> {
        let n = sementes.len();
        let mut passo = 0.0;
        let mut reta = (0.0, 0.0);
        match cfg.tipo {
            TipoSerie::Linear => {
                passo = cfg.incremento.unwrap_or(if n >= 2 { sementes[1] - sementes[0] } else { 1.0 });
            }
            TipoSerie::Geometrica => {
                passo = cfg.incremento.unwrap_or(if n >= 2 {
                    if sementes[0] == 0.0 {
                        return Err(crate::tr!("Série de crescimento: o primeiro valor não pode ser zero.").into());
                    }
                    sementes[1] / sementes[0]
                } else {
                    1.0
                });
            }
            TipoSerie::Data(_) => {
                passo = cfg.incremento.unwrap_or(1.0).round();
            }
            TipoSerie::TendenciaLinear => reta = minimos_quadrados(&sementes),
            TipoSerie::TendenciaExponencial => {
                if sementes.iter().any(|&v| v <= 0.0) {
                    return Err(crate::tr!("Tendência exponencial: os valores selecionados precisam ser positivos.").into());
                }
                let ln: Vec<f64> = sementes.iter().map(|v| v.ln()).collect();
                reta = minimos_quadrados(&ln);
            }
        }
        Ok(Self { sementes, cfg, passo, reta })
    }

    /// Parte da primeira semente, reescrevendo as sementes?
    fn reescreve(&self) -> bool {
        self.cfg.incremento.is_some()
            && matches!(self.cfg.tipo, TipoSerie::Linear | TipoSerie::Geometrica | TipoSerie::Data(_))
    }

    /// Valor da posição `k` (k >= 1).
    fn valor(&self, k: u32, anterior: f64) -> f64 {
        let n = self.sementes.len() as u32;
        let primeira = self.sementes[0];
        let ultima = self.sementes[n as usize - 1];
        let (base, j) = if self.reescreve() { (primeira, k) } else { (ultima, k + 1 - n) };
        match self.cfg.tipo {
            TipoSerie::Linear => base + self.passo * f64::from(j),
            TipoSerie::Geometrica => base * self.passo.powi(j as i32),
            TipoSerie::Data(UnidadeData::Dia) => base + self.passo * f64::from(j),
            TipoSerie::Data(UnidadeData::DiaUtil) => {
                // Anda um passo a partir do valor anterior: fim de semana não conta.
                soma_uteis(anterior as i64, self.passo as i64) as f64
            }
            TipoSerie::Data(UnidadeData::Mes) => soma_meses(base as i64, self.passo as i64 * i64::from(j)) as f64,
            TipoSerie::Data(UnidadeData::Ano) => soma_meses(base as i64, 12 * self.passo as i64 * i64::from(j)) as f64,
            TipoSerie::TendenciaLinear => self.reta.1 + self.reta.0 * f64::from(k + 1),
            TipoSerie::TendenciaExponencial => (self.reta.1 + self.reta.0 * f64::from(k + 1)).exp(),
        }
    }
}

/// Planeja a série sem escrever. Linhas cujas sementes não são todas números
/// ficam como estão. Erro (em pt-BR) só para configuração impossível.
pub fn plan_serie<F>(read: F, origem: &Rect, destino: &Rect, cfg: &ConfigSerie) -> Result<Vec<FillCell>, String>
where
    F: Fn(u32, u32) -> SourceCell,
{
    let origem = origem.normalized();
    let destino = destino.normalized();
    let vertical = cfg.direcao == DirecaoSerie::Colunas;
    let (o_ini, o_fim, grade_max) = if vertical {
        (origem.r0, origem.r1, MAX_ROWS)
    } else {
        (origem.c0, origem.c1, MAX_COLS)
    };
    let d_fim = if vertical { destino.r1 } else { destino.c1 }.max(o_fim);
    let so_limite = d_fim == o_fim;
    if so_limite && cfg.limite.is_none() {
        return Ok(Vec::new());
    }
    if so_limite && matches!(cfg.tipo, TipoSerie::Linear | TipoSerie::Geometrica | TipoSerie::Data(_)) {
        // Sem destino, o limite precisa poder ser alcançado.
        let neutro = match cfg.tipo {
            TipoSerie::Geometrica => cfg.incremento == Some(1.0),
            _ => cfg.incremento == Some(0.0),
        };
        if neutro {
            return Err(crate::tr!("O incremento nunca chega ao valor de parada.").into());
        }
    }
    let teto = if so_limite {
        o_fim.saturating_add(TETO_SEM_DESTINO).min(grade_max - 1)
    } else {
        d_fim.min(grade_max - 1)
    };
    let (l_ini, l_fim) = if vertical { (origem.c0, origem.c1) } else { (origem.r0, origem.r1) };
    let n = o_fim - o_ini + 1;
    let mut saida = Vec::new();

    for linha in l_ini..=l_fim {
        let pos = |p: u32| if vertical { (p, linha) } else { (linha, p) };
        let celulas: Vec<SourceCell> = (o_ini..=o_fim).map(|p| { let (r, c) = pos(p); read(r, c) }).collect();
        let mut sementes = Vec::with_capacity(n as usize);
        for c in &celulas {
            match c.value {
                Value::Number(v) if !c.raw.starts_with('=') => sementes.push(v),
                _ => break,
            }
        }
        if sementes.len() != n as usize {
            continue;
        }
        let g = Gerador::novo(sementes.clone(), *cfg)?;
        let (mut k, mut anterior) = if g.reescreve() { (1, sementes[0]) } else { (n, sementes[n as usize - 1]) };
        let mut anterior_dir = anterior;
        // Dia útil com semente em fim de semana sem reescrever: parte dela.
        while o_ini + k <= teto {
            let v = g.valor(k, anterior);
            if !v.is_finite() {
                break;
            }
            if let Some(lim) = cfg.limite {
                let sobe = v >= anterior_dir;
                if (sobe && v > lim) || (!sobe && v < lim) {
                    break;
                }
            }
            let (r, c) = pos(o_ini + k);
            let estilo = celulas[(k.min(n - 1)) as usize].style.clone();
            let raw = if matches!(cfg.tipo, TipoSerie::Data(_)) { format!("{}", v as i64) } else { format_series(v) };
            saida.push(FillCell { row: r, col: c, raw, style: estilo });
            anterior = v;
            anterior_dir = v;
            k += 1;
        }
    }
    Ok(saida)
}

impl Sheet {
    /// Preencher > Série: planeja ([`plan_serie`]) e escreve numa transação só
    /// (um Ctrl+Z desfaz), com o estilo da semente. Devolve quantas células
    /// escreveu, ou a mensagem do que impede a série.
    pub fn preencher_serie(&mut self, origem: &Rect, destino: &Rect, cfg: &ConfigSerie) -> Result<usize, String> {
        let plano = plan_serie(
            |r, c| {
                let raw = self.get_raw(r, c);
                let value = if raw.starts_with('=') { Value::Empty } else { self.get_value(r, c) };
                SourceCell { raw, value, style: self.estilo_mesclado(r, c) }
            },
            origem,
            destino,
            cfg,
        )?;
        if plano.is_empty() {
            return Ok(0);
        }
        self.abrir_transacao();
        for cel in &plano {
            self.set_cell(cel.row, cel.col, &cel.raw);
            if let Some(estilo) = &cel.style {
                self.set_style(&Rect::new(cel.row, cel.col, cel.row, cel.col), estilo);
            }
        }
        self.fechar_transacao();
        Ok(plano.len())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn lista(valores: &[f64]) -> impl Fn(u32, u32) -> SourceCell + '_ {
        move |r, c| match valores.get(r as usize).filter(|_| c == 0) {
            Some(&v) => SourceCell { raw: v.to_string(), value: Value::Number(v), style: None },
            None => SourceCell { raw: String::new(), value: Value::Empty, style: None },
        }
    }

    fn crus(p: &[FillCell]) -> Vec<&str> {
        p.iter().map(|c| c.raw.as_str()).collect()
    }

    fn col(tipo: TipoSerie) -> ConfigSerie {
        ConfigSerie::nova(tipo, DirecaoSerie::Colunas)
    }

    #[test]
    fn geometrica_deduz_razao_das_sementes() {
        let p = plan_serie(lista(&[2.0, 4.0]), &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 4, 0), &col(TipoSerie::Geometrica)).unwrap();
        assert_eq!(crus(&p), ["8", "16", "32"]);
    }

    #[test]
    fn geometrica_com_incremento_parte_da_primeira_semente() {
        let mut cfg = col(TipoSerie::Geometrica);
        cfg.incremento = Some(2.0);
        // Como o Excel: 2 e 5 selecionados, incremento 2 -> 2, 4, 8, 16.
        let p = plan_serie(lista(&[2.0, 5.0]), &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 3, 0), &cfg).unwrap();
        assert_eq!(crus(&p), ["4", "8", "16"]);
        assert_eq!((p[0].row, p[2].row), (1, 3));
    }

    #[test]
    fn limite_para_a_serie_sem_destino() {
        let mut cfg = col(TipoSerie::Geometrica);
        cfg.incremento = Some(3.0);
        cfg.limite = Some(100.0);
        let p = plan_serie(lista(&[1.0]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 0, 0), &cfg).unwrap();
        assert_eq!(crus(&p), ["3", "9", "27", "81"]);
        // Linear decrescente: 10, 7, 4, 1 (parar em 0).
        let mut lin = col(TipoSerie::Linear);
        lin.incremento = Some(-3.0);
        lin.limite = Some(0.0);
        let p = plan_serie(lista(&[10.0]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 0, 0), &lin).unwrap();
        assert_eq!(crus(&p), ["7", "4", "1"]);
        // Limite também corta um destino maior.
        lin.incremento = Some(1.0);
        lin.limite = Some(13.0);
        let p = plan_serie(lista(&[10.0]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 9, 0), &lin).unwrap();
        assert_eq!(crus(&p), ["11", "12", "13"]);
    }

    #[test]
    fn incremento_que_nunca_chega_ao_limite_e_erro() {
        let mut cfg = col(TipoSerie::Linear);
        cfg.incremento = Some(0.0);
        cfg.limite = Some(5.0);
        assert!(plan_serie(lista(&[1.0]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 0, 0), &cfg).is_err());
    }

    #[test]
    fn tendencia_linear_minimos_quadrados() {
        // 1, 3, 8: inclinação 3,5; intercepto -3 -> x=4: 11, x=5: 14,5.
        let p = plan_serie(lista(&[1.0, 3.0, 8.0]), &Rect::new(0, 0, 2, 0), &Rect::new(0, 0, 4, 0), &col(TipoSerie::TendenciaLinear)).unwrap();
        assert_eq!(crus(&p), ["11", "14.5"]);
    }

    #[test]
    fn tendencia_exponencial() {
        let p = plan_serie(lista(&[2.0, 4.0, 8.0]), &Rect::new(0, 0, 2, 0), &Rect::new(0, 0, 4, 0), &col(TipoSerie::TendenciaExponencial)).unwrap();
        assert_eq!(crus(&p), ["16", "32"]);
        assert!(plan_serie(lista(&[2.0, -4.0]), &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 3, 0), &col(TipoSerie::TendenciaExponencial)).is_err());
    }

    #[test]
    fn tendencia_com_limite() {
        let mut cfg = col(TipoSerie::TendenciaLinear);
        cfg.limite = Some(20.0);
        let p = plan_serie(lista(&[1.0, 3.0, 8.0]), &Rect::new(0, 0, 2, 0), &Rect::new(0, 0, 2, 0), &cfg).unwrap();
        assert_eq!(crus(&p), ["11", "14.5", "18", "21.5"].iter().take(3).copied().collect::<Vec<_>>());
    }

    fn data(a: i32, m: i32, d: i32) -> f64 {
        date_to_serial(a, m, d)
    }

    #[test]
    fn dias_uteis_a_partir_de_sexta_pulam_o_fim_de_semana() {
        // 16/10/2026 é sexta.
        let sexta = data(2026, 10, 16);
        assert_eq!(dia_da_semana(sexta as i64), 5);
        let cfg = col(TipoSerie::Data(UnidadeData::DiaUtil));
        let p = plan_serie(lista(&[sexta]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 3, 0), &cfg).unwrap();
        let esperado: Vec<String> = [(19, 10), (20, 10), (21, 10)].iter().map(|&(d, m)| format!("{}", data(2026, m, d) as i64)).collect();
        assert_eq!(crus(&p), esperado.iter().map(String::as_str).collect::<Vec<_>>());
    }

    #[test]
    fn por_mes_e_por_ano_com_dia_ajustado() {
        let cfg = col(TipoSerie::Data(UnidadeData::Mes));
        let p = plan_serie(lista(&[data(2026, 1, 31)]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 3, 0), &cfg).unwrap();
        let esp: Vec<String> = [data(2026, 2, 28), data(2026, 3, 31), data(2026, 4, 30)].iter().map(|v| format!("{}", *v as i64)).collect();
        assert_eq!(crus(&p), esp.iter().map(String::as_str).collect::<Vec<_>>());
        let ano = col(TipoSerie::Data(UnidadeData::Ano));
        let p = plan_serie(lista(&[data(2024, 2, 29)]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 2, 0), &ano).unwrap();
        let esp: Vec<String> = [data(2025, 2, 28), data(2026, 2, 28)].iter().map(|v| format!("{}", *v as i64)).collect();
        assert_eq!(crus(&p), esp.iter().map(String::as_str).collect::<Vec<_>>());
    }

    #[test]
    fn data_com_limite_e_incremento_de_dois_meses() {
        let mut cfg = col(TipoSerie::Data(UnidadeData::Mes));
        cfg.incremento = Some(2.0);
        cfg.limite = Some(data(2026, 7, 1));
        let p = plan_serie(lista(&[data(2026, 1, 1)]), &Rect::new(0, 0, 0, 0), &Rect::new(0, 0, 0, 0), &cfg).unwrap();
        assert_eq!(p.len(), 3); // março, maio, julho
    }

    #[test]
    fn em_linhas_vai_para_a_direita_e_linha_com_texto_e_ignorada() {
        let ler = |r: u32, c: u32| match (r, c) {
            (0, 0) => SourceCell { raw: "1".into(), value: Value::Number(1.0), style: None },
            (1, 0) => SourceCell { raw: "a".into(), value: Value::Text("a".into()), style: None },
            _ => SourceCell { raw: String::new(), value: Value::Empty, style: None },
        };
        let mut cfg = ConfigSerie::nova(TipoSerie::Geometrica, DirecaoSerie::Linhas);
        cfg.incremento = Some(2.0);
        let p = plan_serie(ler, &Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 1, 3), &cfg).unwrap();
        assert_eq!(crus(&p), ["2", "4", "8"]);
        assert!(p.iter().all(|c| c.row == 0));
        assert_eq!(p[0].col, 1);
    }

    #[test]
    fn na_planilha_escreve_e_desfaz_numa_transacao() {
        let mut s = Sheet::new("A");
        s.set_cell(0, 0, "2");
        s.set_cell(1, 0, "4");
        let cfg = col(TipoSerie::Geometrica);
        let feitas = s.preencher_serie(&Rect::new(0, 0, 1, 0), &Rect::new(0, 0, 4, 0), &cfg).unwrap();
        assert_eq!(feitas, 3);
        assert_eq!(s.get_raw(4, 0), "32");
        assert_eq!(s.get_value(3, 0), Value::Number(16.0));
        assert!(s.undo());
        assert_eq!(s.get_raw(2, 0), "");
        assert_eq!(s.get_raw(1, 0), "4");
        assert!(s.redo());
        assert_eq!(s.get_raw(4, 0), "32");
    }
}
