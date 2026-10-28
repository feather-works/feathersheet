//! Planejamento da Ribbon: compressão por prioridade, não por posição.
//! O nível de cada seção é independente; manter fonte/alinhamento e comandos
//! de uso frequente visíveis é mais importante do que sua ordem na faixa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Nivel { Completo, Compacto, Recolhido }

impl Nivel {
    const fn indice(self) -> usize {
        match self { Self::Completo=>0, Self::Compacto=>1, Self::Recolhido=>2 }
    }
}

/// Seções (ordem visual): Área de transferência, Fonte, Alinhamento,
/// Número, Estilos, Células e Edição.
pub(super) const NOMES: [&str;7] = [
    "Área de transferência", "Fonte", "Alinhamento", "Número",
    "Estilos", "Células", "Edição"
];

/// Larguras medidas na tela (27/10/2026) com folga de poucos pixels.
/// O layout real precisa usar widgets que caibam nos limites compactos.
const LARGURAS: [[f32;3];7] = [
    [182., 166., 78.],
    [294., 256., 86.],
    [320., 208., 92.],
    [198., 153., 78.],
    [380., 160., 76.],
    [228., 131., 75.],
    [263., 136., 75.],
];
// Primeiro todas as seções ficam compactas (empilhadas); só depois alguma
// vira um botão só. O grupo Fonte é o último a encolher em cada etapa.
const PASSOS: [(usize, Nivel);14] = [
    (4, Nivel::Compacto), (5, Nivel::Compacto), (6, Nivel::Compacto),
    (0, Nivel::Compacto), (3, Nivel::Compacto), (2, Nivel::Compacto),
    (1, Nivel::Compacto),
    (4, Nivel::Recolhido), (5, Nivel::Recolhido), (6, Nivel::Recolhido),
    (0, Nivel::Recolhido), (3, Nivel::Recolhido),
    (2, Nivel::Recolhido), (1, Nivel::Recolhido),
];
const RESERVA_MAIS: f32 = 94.;
const MARGENS: f32 = 18.;
const ENTRE_GRUPOS: f32 = 2.;

#[derive(Debug,Clone)]
pub(super) struct Plano {
    pub niveis: [Nivel;7],
    pub visiveis: usize,
}
impl Plano {
    pub fn largura_antes(&self, i:usize) -> f32 {
        self.niveis.iter().take(i).enumerate()
            .map(|(j,n)|LARGURAS[j][n.indice()]+ENTRE_GRUPOS)
            .sum()
    }
    // Utilizado apenas nas asserções dos testes de responsividade.
    #[cfg(test)]
    pub fn largura_total(&self) -> f32 {
        self.niveis.iter().take(self.visiveis).enumerate()
            .map(|(j,n)| LARGURAS[j][n.indice()] + if j>0 {ENTRE_GRUPOS} else {0.})
            .sum()
    }
}

fn total(niveis:&[Nivel;7], visiveis:usize)->f32 {
    niveis.iter().take(visiveis).enumerate().map(|(i,n)|
        LARGURAS[i][n.indice()] + if i>0 {ENTRE_GRUPOS} else {0.}
    ).sum()
}

/// Sem reserva artificial de "Mais" até que efetivamente existam itens fora.
/// Janelas >=760 px usam menus individuais, nunca o popup gigantesco.
pub(super) fn planejar(largura_janela:f32)->Plano {
    let disponivel=(largura_janela - MARGENS).max(0.);
    let mut niveis=[Nivel::Completo;7];
    for &(i,n) in &PASSOS {
        if total(&niveis,7)<=disponivel { break; }
        niveis[i]=n;
    }
    if total(&niveis,7)<=disponivel {
        return Plano{niveis,visiveis:7};
    }
    // Somente telas excepcionalmente estreitas perdem seções completas.
    let limite=(disponivel - RESERVA_MAIS).max(0.);
    let mut quantidade=0;
    for i in 0..7 {
        if total(&niveis,i+1)>limite { break; }
        quantidade=i+1;
    }
    Plano{niveis,visiveis:quantidade.max(1)}
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn inicio_responsivo_prioriza_fonte_e_evitar_menu_geral_em_desktop() {
        let p=planejar(1280.);
        assert_eq!(p.visiveis,7,"1280 px deve mostrar sete seções");
        assert_eq!(p.niveis[1],Nivel::Completo,"fonte deve ser prioridade");
        assert!(p.niveis[4]!=Nivel::Completo,"estilos perde largura primeiro");
        assert!(p.largura_total()<=1280.-MARGENS);
        for width in [760., 850., 1024., 1280., 1440., 1920., 2560.] {
            let p=planejar(width);
            assert_eq!(p.visiveis,7,"não abrir Mais em {width}px");
            assert!(p.largura_total()<=width-MARGENS);
        }
    }
    #[test]
    fn perder_prioridades_e_monotono_ao_diminuir_janela() {
        let mut prev=planejar(2560.);
        for w in (500..=2550).rev().step_by(10) {
            let p=planejar(w as f32);
            for i in 0..7 {
                assert!(p.niveis[i].indice()>=prev.niveis[i].indice(),
                    "{w}px: grupo {i} expandiu ao reduzir largura");
            }
            assert!(p.visiveis<=prev.visiveis);
            prev=p;
        }
        assert!(planejar(510.).visiveis < 7,
            "a única exceção para o menu geral é uma janela estreita");
    }
    #[test]
    fn todos_grupos_sao_acessiveis_com_reserva_de_mais_em_tela_estreita() {
        let p=planejar(430.);
        assert!(p.visiveis>=1&&p.visiveis<7);
        assert_eq!(NOMES.len(),p.niveis.len());
        assert!(p.largura_antes(5)>p.largura_antes(2));
    }
}
