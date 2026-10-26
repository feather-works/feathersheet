//! Ponto de troca com o catálogo comum `feather-shapes`: a única parte da
//! camada de objetos que sabe desenhar uma forma.
//!
//! - O id gravado no `.fshz` é o [`TipoForma::id`] do `feather-shapes`
//!   (`retangulo`, `seta-direita`…), o mesmo do FeatherSlide; `prstGeom`
//!   (`rect`, `rightArrow`…) também é aceito na leitura.
//! - Os ajustes seguem o sentido do Slide/`feather-shapes`: `retangulo` =
//!   4 raios (fração do lado menor, um por canto); `retangulo-arredondado`
//!   = `[topo, base]`.
//! - **Ponte provisória:** a `feather-shapes` v0.1.0 ainda não lê ajustes de
//!   seta, estrela e triângulo. Enquanto não lê, [`desenho`] usa a geometria
//!   local de [`com_ajuste_local`] SÓ quando essas formas têm ajuste; sem
//!   ajuste, tudo vem do crate. Quando o crate ler esses ajustes, apagar
//!   `com_ajuste_local` e nada mais muda.
use feather_draw::{Caminho, Comando, Ponto, Retangulo};
use feather_shapes::tipo::TipoForma;

/// O tipo pelo id gravado (id do catálogo ou `prstGeom`).
pub(in crate::app) fn tipo(id: &str) -> Option<TipoForma> {
    TipoForma::pelo_id(id).or_else(|| TipoForma::pelo_prst(id))
}

/// Nome em pt-BR (o próprio id se não estiver no catálogo).
pub(in crate::app) fn nome(id: &str) -> String {
    tipo(id).map_or_else(|| id.to_owned(), |t| t.nome().to_owned())
}

/// Nome para a barra de status: "Ícone home" ou o nome da forma.
pub(in crate::app) fn nome_objeto(o: &feathersheet_io::objetos_aba::ObjetoSolto) -> String {
    match o.tipo {
        feathersheet_io::objetos_aba::TipoObjeto::Icone => trf!("Ícone {}", o.forma.replace('-', " ")),
        feathersheet_io::objetos_aba::TipoObjeto::Wordart => "WordArt".to_owned(),
        _ => nome(&o.forma),
    }
}

/// A forma é só traço (linha, conector, arco)?
pub(in crate::app) fn so_traco(id: &str) -> bool {
    tipo(id).is_some_and(TipoForma::so_traco)
}

/// O catálogo do menu Formas: `(categoria, tipos)` na ordem do crate.
pub(in crate::app) fn catalogo() -> Vec<(&'static str, Vec<TipoForma>)> {
    let mut grupos: Vec<(&'static str, Vec<TipoForma>)> = Vec::new();
    for &t in TipoForma::TODOS {
        let nome = t.categoria().nome();
        match grupos.iter_mut().find(|(n, _)| *n == nome) {
            Some((_, v)) => v.push(t),
            None => grupos.push((nome, vec![t])),
        }
    }
    grupos
}

/// O desenho de uma forma: regiões preenchidas e detalhes só de traço.
pub(in crate::app) struct Desenho {
    pub preenchida: Caminho,
    pub tracos: Caminho,
}

/// O desenho da forma `id` com `ajustes` dentro de `r` (sem giro nem
/// espelho). Forma desconhecida vira retângulo, para nunca sumir.
pub(in crate::app) fn desenho(id: &str, ajustes: &[f32], r: Retangulo) -> Desenho {
    let t = tipo(id).unwrap_or(TipoForma::Retangulo);
    if !ajustes.is_empty() {
        if let Some(c) = com_ajuste_local(t, ajustes, r) {
            return Desenho { preenchida: c, tracos: Caminho::novo() };
        }
    }
    match feather_shapes::forma::geometria(t, ajustes, r) {
        Some(g) => Desenho { preenchida: g.preenchida, tracos: g.tracos },
        None => Desenho { preenchida: Caminho::novo(), tracos: Caminho::novo() },
    }
}

fn poligono(pontos: &[(f32, f32)]) -> Caminho {
    let mut c = Caminho::novo();
    for (i, &(x, y)) in pontos.iter().enumerate() {
        let p = Ponto::novo(x, y);
        c.adicionar(if i == 0 { Comando::Mover(p) } else { Comando::Linha(p) });
    }
    c.adicionar(Comando::Fechar);
    c
}

/// Ponte provisória (ver o topo do módulo): seta, estrela e triângulo com ajuste.
fn com_ajuste_local(t: TipoForma, a: &[f32], r: Retangulo) -> Option<Caminho> {
    let Retangulo { x, y, largura: w, altura: h } = r;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    Some(match t {
        TipoForma::SetaDireita => {
            let haste = a.first().copied().unwrap_or(0.4).clamp(0.05, 1.0);
            let ponta = a.get(1).copied().unwrap_or(0.4).clamp(0.0, 1.0) * w;
            let (topo, base) = (cy - h * haste / 2.0, cy + h * haste / 2.0);
            let px = x + w - ponta;
            poligono(&[(x, topo), (px, topo), (px, y), (x + w, cy), (px, y + h), (px, base), (x, base)])
        }
        TipoForma::Estrela5 => {
            let f = a.first().copied().unwrap_or(0.44).clamp(0.05, 0.95);
            let pontos: Vec<(f32, f32)> = (0..10)
                .map(|i| {
                    let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let k = if i % 2 == 0 { 1.0 } else { f };
                    (cx + w / 2.0 * k * ang.cos(), cy + h / 2.0 * k * ang.sin())
                })
                .collect();
            poligono(&pontos)
        }
        TipoForma::Triangulo => {
            let v = a.first().copied().unwrap_or(0.5).clamp(0.0, 1.0);
            poligono(&[(x + w * v, y), (x + w, y + h), (x, y + h)])
        }
        _ => return None,
    })
}

// ------------------------------------------------------------ alças de ajuste

/// Folga da alça de canto para dentro do raio, em px de tela (como no Slide).
const FOLGA_CANTO: f32 = 14.0;
/// Índice da alça "todos os cantos" (raio único).
pub(in crate::app) const TODOS_CANTOS: usize = 4;

/// Os quatro raios (sup. esq., sup. dir., inf. dir., inf. esq.) como fração
/// do lado menor, no sentido do Slide. `None` = forma sem cantos ajustáveis.
pub(in crate::app) fn raios(id: &str, a: &[f32]) -> Option<[f32; 4]> {
    match tipo(id)? {
        TipoForma::Retangulo => Some(if a.len() == 4 { [a[0], a[1], a[2], a[3]] } else { [0.0; 4] }),
        TipoForma::RetanguloArredondado => {
            let cima = a.first().copied().unwrap_or(0.16);
            let baixo = a.get(1).copied().unwrap_or(cima);
            Some([cima, cima, baixo, baixo])
        }
        _ => None,
    }
}

/// O id e os ajustes que desenham os raios pedidos (roundRect vira
/// retângulo com raio por canto quando os cantos ficam diferentes).
pub(in crate::app) fn com_raios(id: &str, raios: [f32; 4]) -> (&'static str, Vec<f32>) {
    let r = raios.map(|v| (v.clamp(0.0, 0.5) * 1000.0).round() / 1000.0);
    if tipo(id) == Some(TipoForma::RetanguloArredondado) && r[0] == r[1] && r[2] == r[3] {
        return (TipoForma::RetanguloArredondado.id(), vec![r[0], r[2]]);
    }
    (TipoForma::Retangulo.id(), r.to_vec())
}

/// As alças de ajuste da forma em `(x, y, w, h)` da tela (sem giro):
/// `(índice, ponto)`. Retângulo com raio único tem uma alça (`TODOS_CANTOS`);
/// com raios diferentes, uma por canto (0–3).
pub(in crate::app) fn alcas(id: &str, a: &[f32], (x, y, w, h): (f32, f32, f32, f32)) -> Vec<(usize, (f32, f32))> {
    let m = w.abs().min(h.abs());
    if let Some(rs) = raios(id, a) {
        let d = |r: f32| (r * m + FOLGA_CANTO).min(w / 2.0);
        if rs.iter().all(|&r| (r - rs[0]).abs() < 1e-4) {
            return vec![(TODOS_CANTOS, (x + d(rs[0]), y))];
        }
        return vec![
            (0, (x + d(rs[0]), y)),
            (1, (x + w - d(rs[1]), y)),
            (2, (x + w - d(rs[2]), y + h)),
            (3, (x + d(rs[3]), y + h)),
        ];
    }
    let Some(t) = tipo(id) else { return Vec::new() };
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    match t {
        TipoForma::SetaDireita => {
            let [haste, ponta] = ajustes_seta(a, w, h);
            vec![(0, (x, cy - h * haste / 2.0)), (1, (x + w - ponta * w, y))]
        }
        TipoForma::Estrela5 => {
            let f = a.first().copied().unwrap_or(0.44);
            let ang = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI / 5.0;
            vec![(0, (cx + w / 2.0 * f * ang.cos(), cy + h / 2.0 * f * ang.sin()))]
        }
        TipoForma::Triangulo => vec![(0, (x + w * a.first().copied().unwrap_or(0.5), y))],
        _ => Vec::new(),
    }
}

/// Ajustes atuais da seta (com o padrão do crate quando vazio).
fn ajustes_seta(a: &[f32], w: f32, h: f32) -> [f32; 2] {
    let padrao_ponta = if w > 0.0 { (w * 0.4).min(h) / w } else { 0.4 };
    [a.first().copied().unwrap_or(0.4), a.get(1).copied().unwrap_or(padrao_ponta)]
}

/// O ajuste que o cursor `q` (tela, sem giro) pede na alça `indice`.
/// `por_canto` (Ctrl) solta o canto 0 dos outros. Devolve o id (pode
/// mudar de retângulo arredondado para retângulo) e os ajustes novos.
pub(in crate::app) fn ajustar(
    id: &str, a: &[f32], indice: usize, q: (f32, f32), (x, y, w, h): (f32, f32, f32, f32), por_canto: bool,
) -> Option<(&'static str, Vec<f32>)> {
    let m = w.abs().min(h.abs()).max(1.0);
    if let Some(mut rs) = raios(id, a) {
        let da_borda = |i: usize| if i == 0 || i == 3 { q.0 - x } else { x + w - q.0 };
        let canto = if indice == TODOS_CANTOS && por_canto { 0 } else { indice };
        let r = ((da_borda(canto.min(3)) - FOLGA_CANTO) / m).clamp(0.0, 0.5);
        if canto == TODOS_CANTOS { rs = [r; 4] } else { rs[canto] = r }
        return Some(com_raios(id, rs));
    }
    let t = tipo(id)?;
    let tid = t.id();
    Some(match (t, indice) {
        (TipoForma::SetaDireita, i) => {
            let mut s = ajustes_seta(a, w, h);
            if i == 0 {
                s[0] = (2.0 * (y + h / 2.0 - q.1).abs() / h.max(1.0)).clamp(0.05, 1.0);
            } else {
                s[1] = ((x + w - q.0) / w.max(1.0)).clamp(0.0, 1.0);
            }
            (tid, s.map(|v| (v * 1000.0).round() / 1000.0).to_vec())
        }
        (TipoForma::Estrela5, _) => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let f = ((q.0 - cx) / (w / 2.0).max(1.0)).hypot((q.1 - cy) / (h / 2.0).max(1.0));
            (tid, vec![(f.clamp(0.05, 0.95) * 1000.0).round() / 1000.0])
        }
        (TipoForma::Triangulo, _) => (tid, vec![(((q.0 - x) / w.max(1.0)).clamp(0.0, 1.0) * 1000.0).round() / 1000.0]),
        _ => return None,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_o_catalogo_desenha_dentro_da_caixa() {
        let r = Retangulo::novo(10.0, 20.0, 120.0, 80.0);
        let grupos = catalogo();
        assert!(grupos.len() >= 8);
        assert_eq!(grupos.iter().map(|(_, v)| v.len()).sum::<usize>(), TipoForma::TODOS.len());
        for &t in TipoForma::TODOS {
            let d = desenho(t.id(), &[], r);
            assert!(!d.preenchida.comandos.is_empty() || !d.tracos.comandos.is_empty(), "{} sem desenho", t.id());
        }
        // prstGeom também abre; desconhecida vira retângulo.
        assert_eq!(tipo("rightArrow"), Some(TipoForma::SetaDireita));
        assert!(!desenho("nao-existe", &[], r).preenchida.comandos.is_empty());
        assert!(so_traco("linha") && !so_traco("retangulo"));
        assert_eq!(nome("estrela-5"), "Estrela");
    }

    #[test]
    fn alca_de_canto_arredonda_todos_e_ctrl_so_um() {
        let caixa = (0.0, 0.0, 200.0, 100.0);
        let al = alcas("retangulo", &[], caixa);
        assert_eq!(al, vec![(TODOS_CANTOS, (FOLGA_CANTO, 0.0))]);
        // Arrastar 20 px para dentro: raio de 20/100 em todos os cantos.
        let (id, a) = ajustar("retangulo", &[], TODOS_CANTOS, (FOLGA_CANTO + 20.0, 0.0), caixa, false).unwrap();
        assert_eq!((id, a.clone()), ("retangulo", vec![0.2; 4]));
        assert_eq!(alcas(id, &a, caixa).len(), 1);
        // Ctrl: só o canto superior esquerdo; aparecem as quatro alças.
        let (id, a) = ajustar(id, &a, TODOS_CANTOS, (FOLGA_CANTO + 40.0, 0.0), caixa, true).unwrap();
        assert_eq!(a, vec![0.4, 0.2, 0.2, 0.2]);
        assert_eq!(alcas(id, &a, caixa).len(), 4);
        // Arredondado com cantos iguais continua arredondado ([topo, base]).
        let (id, a) = ajustar("retangulo-arredondado", &[], TODOS_CANTOS, (FOLGA_CANTO + 10.0, 0.0), caixa, false).unwrap();
        assert_eq!((id, a), ("retangulo-arredondado", vec![0.1, 0.1]));
        assert!(raios("elipse", &[]).is_none());
    }

    #[test]
    fn seta_estrela_e_triangulo_tem_ajuste_e_desenham_com_ele() {
        let caixa = (0.0, 0.0, 200.0, 100.0);
        let r = Retangulo::novo(0.0, 0.0, 200.0, 100.0);
        let (_, a) = ajustar("seta-direita", &[], 0, (0.0, 20.0), caixa, false).unwrap();
        assert!((a[0] - 0.6).abs() < 1e-3, "{a:?}");
        let (_, a) = ajustar("seta-direita", &a, 1, (120.0, 0.0), caixa, false).unwrap();
        assert!((a[1] - 0.4).abs() < 1e-3, "{a:?}");
        assert_eq!(desenho("seta-direita", &a, r).preenchida.comandos.len(), 8);
        let (_, e) = ajustar("estrela-5", &[], 0, (100.0, 25.0), caixa, false).unwrap();
        assert!((e[0] - 0.5).abs() < 1e-3);
        assert_eq!(alcas("estrela-5", &e, caixa).len(), 1);
        let (_, t) = ajustar("triangulo", &[], 0, (50.0, 0.0), caixa, false).unwrap();
        assert_eq!(t, vec![0.25]);
        assert_eq!(alcas("triangulo", &t, caixa), vec![(0, (50.0, 0.0))]);
        assert!(alcas("elipse", &[], caixa).is_empty());
    }
}
