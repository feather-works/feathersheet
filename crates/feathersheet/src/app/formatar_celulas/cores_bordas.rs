//! Abas Preenchimento e Borda: controles visuais, prévia e seletor HSV compartilhado.
use super::*;

fn swatch(cor: &str, largura: f32, altura: f32) -> Element<'static, Msg> {
    let cor=if seletor_cor::hex_valido(cor) {
        let [r,g,b,a]=seletor_cor::rgba_de(cor);
        pelicano::Color::rgb8(r, g, b).alpha(a as f32/255.)
    } else {pelicano::Color::TRANSPARENT};
    container(Space::new(Length::Fixed(largura),Length::Fixed(altura)))
        .style(move |_: &pelicano::Theme| pelicano::Style {
            background:Some(pelicano::Background::Color(cor)),
            border:pelicano::Border {color:COR_BORDA(),width:1.,radius: [3.0; 4]},
            ..Default::default()
        }).into()
}

/// O SVG montado aqui, no tamanho pedido (sempre válido; se não, o espaço vazio).
fn svg_de(svg: &str, largura: f32, altura: f32) -> Element<'static, Msg> {
    match pelicano::Svg::from_bytes(svg.as_bytes()) {
        Some(s) => pelicano::svg(s).width(Length::Fixed(largura)).height(Length::Fixed(altura)).into(),
        None => Space::new(Length::Fixed(largura), Length::Fixed(altura)).into(),
    }
}

fn borda_ativa(b: &BTreeMap<String, Campo>,lado: &str) -> bool {
    matches!(b.get(lado),Some(Campo::Logico(true)))
}

/// A prévia reflete imediatamente as escolhas do rascunho.
fn desenho_previa(e: &Estado) -> Element<'static, Msg> {
    let b=e.bordas();
    let rgb=if seletor_cor::hex_valido(&e.cor_borda) {
        e.cor_borda.get(..7).unwrap_or("#808080")
    } else {"#808080"};
    let grossura=match e.texto("borderLineStyle") {
        "Média"=>2.5, "Grossa"=>4.,_=>1.5
    };
    let tracejado=if e.texto("borderLineStyle")=="Tracejada" {
        "stroke-dasharray=\"7 5\"" 
    } else {""};
    let mut linhas=String::new();
    for (lado,geom) in [
        ("top","M18 18H158"), ("right","M158 18V132"),
        ("bottom","M18 132H158"), ("left","M18 18V132")
    ] {
        if borda_ativa(&b,lado) {
            linhas.push_str(&format!(
                "<path d=\"{geom}\" stroke=\"{rgb}\" stroke-width=\"{grossura}\" {tracejado}/>"
            ));
        }
    }
    let svg=format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="176" height="150" viewBox="0 0 176 150" fill="none">
        <rect x="18" y="18" width="140" height="114" fill="#ffffff" fill-opacity="0.25" stroke="#a1aab4" stroke-width="1" stroke-dasharray="4 4"/>
        <path d="M88 18V132 M18 75H158" stroke="#a1aab4" stroke-width="1" stroke-dasharray="3 4"/>
        <text x="88" y="79" fill="#808080" text-anchor="middle" font-size="11" font-family="sans-serif">Texto</text>
        {linhas}
    </svg>"##);
    svg_de(&svg, 176., 150.)
}

fn opcao_borda(e: &Estado, lado: &'static str, nome: &'static str) -> Element<'static, Msg> {
    let b=e.bordas();
    let ativo=borda_ativa(&b,lado);
    let icone_nome=match lado {
        // Chave do desenho em português: não traduzir (ver menu_bordas::desenho_borda).
        "top"=>"Superior","bottom"=>"Inferior","left"=>"Esquerda","right"=>"Direita",_=>"Externas"
    };
    button(pelicano::row![
        crate::app::menu_bordas::icone_borda(icone_nome,20.),
        text(nome).size(12).no_wrap()
    ].spacing(6).align(pelicano::Align::Center))
        .padding([6,9]).style(estilo_botao_pequeno(ativo))
        .on_press(Msg::FormatoCelula(Evento::Borda(lado))).into()
}

pub(in crate::app) fn seletor_no_dialogo(e: &Estado, titulo: &'static str) -> Element<'static, Msg> {
    feather_ui::seletor_cor::painel_embutido(
        titulo, e.cor_atual(), e.cor_hsva, &e.cor_hex_picker
    ).map(|evento| Msg::FormatoCelula(Evento::CorSeletor(evento)))
}

/// A cor da aba em pouco espaço: amostra da atual, a paleta em duas
/// fileiras, "Sem cor" e "Mais cores…" (o seletor completo só quando pedido).
pub(in crate::app) fn cores_compactas(e: &Estado, sem: &'static str) -> Element<'static, Msg> {
    let atual = e.cor_atual().to_owned();
    let mut grade = pelicano::column![].spacing(4);
    for fileira in CORES.chunks(12) {
        let mut linha = pelicano::row![].spacing(4);
        for &hex in fileira {
            let cor = seletor_cor::rgba_de(hex);
            let escolhida = atual.eq_ignore_ascii_case(hex);
            linha = linha.push(button(Space::new(Length::Fixed(16.), Length::Fixed(16.))).padding(0)
                .style(move |_, status| pelicano::Style {
                    background: Some(pelicano::Background::Color(pelicano::Color::rgb8(cor[0], cor[1], cor[2]))),
                    border: pelicano::Border {
                        color: if escolhida || matches!(status, pelicano::Status::Hovered) { COR_AZUL_ACENTO() } else { COR_BORDA() },
                        width: if escolhida { 2.5 } else { 1. },
                        radius: [4.0; 4],
                    },
                    ..Default::default()
                })
                .on_press(Msg::FormatoCelula(Evento::CorPredefinida(hex))));
        }
        grade = grade.push(linha);
    }
    let nome = if atual.is_empty() { sem.to_owned() } else { atual.clone() };
    pelicano::column![
        grade,
        pelicano::row![
            swatch(&atual, 18., 18.),
            text(nome).size(12).width(Length::FILL),
            button(text(tr!("Sem cor")).size(12).no_wrap()).padding([5, 9]).style(estilo_botao_secundario)
                .on_press(Msg::FormatoCelula(Evento::Texto(e.cor_alvo.campo(), String::new()))),
            button(pelicano::row![feather_ui::icones::icone("palette", 14.), text(tr!("Mais cores…")).size(12).no_wrap()]
                .spacing(6).align(pelicano::Align::Center))
                .padding([5, 9]).style(estilo_botao_pequeno(e.mais_cores))
                .on_press(Msg::FormatoCelula(Evento::MaisCores)),
        ].spacing(8).align(pelicano::Align::Center),
    ].spacing(8).into()
}

/// Título de seção dos painéis (caixa alta, discreto), igual nas abas.
pub(in crate::app) fn secao<'a>(t: &str) -> Element<'a, Msg> {
    text(t.to_owned()).size(11).color(COR_TEXTO_MUTED()).into()
}

impl App {
    /// Borda: à esquerda predefinições, estilo e cor; à direita a célula
    /// com os quatro lados clicáveis (ou o seletor completo, se aberto).
    pub(in crate::app) fn formato_borda(&self, e: &Estado) -> Element<'_, Msg> {
        let mut presets=pelicano::row![].spacing(5);
        // O motor de bordas atual trabalha com quatro lados; não oferecer
        // aqui presets "internas" que simulavam incorretamente bordas externas.
        // A chave (ícone e evento) fica em português; só o texto é traduzido.
        for nome in ["Nenhuma", "Externas", "Todas"] {
            presets=presets.push(
                button(pelicano::row![
                    crate::app::menu_bordas::icone_borda(nome,18.),
                    text(tr!(nome)).size(12),
                ].spacing(5).align(pelicano::Align::Center))
                .style(estilo_botao_pequeno(false)).padding([6,8])
                .on_press(Msg::FormatoCelula(Evento::Borda(nome)))
            );
        }
        let mut estilo_linhas=pelicano::row![].spacing(5);
        for nome in ["Fina","Média","Grossa","Tracejada"] {
            let selected=e.texto("borderLineStyle")==nome;
            let size=match nome {"Grossa"=>4.,"Média"=>2.5,_=>1.3};
            let dash=if nome=="Tracejada" {"stroke-dasharray=\"5 3\""} else {""};
            let svg=format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="35" height="24" viewBox="0 0 35 24"><path d="M3 12H32" stroke="#808080" stroke-width="{size}" {dash}/></svg>"##);
            let img=svg_de(&svg, 34., 18.);
            estilo_linhas=estilo_linhas.push(
                button(pelicano::column![img,text(crate::app::menu_bordas::nome_estilo_linha(nome)).size(11)]
                    .spacing(1).align(pelicano::Align::Center))
                    .padding([4,8]).style(estilo_botao_pequeno(selected))
                    .on_press(Msg::FormatoCelula(Evento::Borda(nome)))
            );
        }
        let ajustes=pelicano::column![
            secao(tr!("PREDEFINIÇÕES")),
            presets,
            secao(tr!("ESTILO DA LINHA")),
            estilo_linhas,
            secao(tr!("COR DA BORDA")),
            cores_compactas(e, tr!("Automática")),
        ].spacing(8).width(Length::FILL);
        let direita: Element<'_, Msg> = if e.mais_cores {
            seletor_no_dialogo(e,tr!("Cor da borda"))
        } else {
            pelicano::column![
                secao(tr!("PRÉVIA DA CÉLULA")),
                container(opcao_borda(e,"top",tr!("Acima"))).width(Length::FILL).align_x(pelicano::Align::Center),
                pelicano::row![
                    opcao_borda(e,"left",tr!("Esquerda")),
                    desenho_previa(e),
                    opcao_borda(e,"right",tr!("Direita")),
                ].spacing(4).align(pelicano::Align::Center),
                container(opcao_borda(e,"bottom",tr!("Abaixo"))).width(Length::FILL).align_x(pelicano::Align::Center),
                text(tr!("Clique nos lados para pôr ou tirar a borda."))
                    .size(11).color(COR_TEXTO_MUTED()),
            ].spacing(4).into()
        };
        pelicano::row![
            container(ajustes).padding(12).width(Length::FILL).style(estilo_painel_faixa),
            container(direita).padding(12).width(Length::Fixed(420.)).style(estilo_painel_faixa),
        ].spacing(12).align(pelicano::Align::Start).into()
    }

    /// Preenchimento: a paleta grande à esquerda; à direita a prévia (ou o
    /// seletor completo, se aberto).
    pub(in crate::app) fn formato_preenchimento(&self, e: &Estado) -> Element<'_, Msg> {
        let mut paleta=pelicano::column![].spacing(6);
        for bloco in CORES.chunks(6) {
            let mut linha=pelicano::row![].spacing(6);
            for &hex in bloco {
                let cor=seletor_cor::rgba_de(hex);
                let escolhida=e.cor_fundo.eq_ignore_ascii_case(hex);
                linha=linha.push(
                    button(Space::new(Length::Fixed(30.),Length::Fixed(22.))).padding(2)
                        .style(move |_,status| pelicano::Style {
                            background:Some(pelicano::Background::Color(pelicano::Color::rgb8(cor[0],cor[1],cor[2]))),
                            border:pelicano::Border {
                                color:if escolhida || matches!(status, pelicano::Status::Hovered) {COR_AZUL_ACENTO()} else {COR_BORDA()},
                                width:if escolhida {2.5} else {1.},radius: [5.0; 4],
                            },
                            ..Default::default()
                        })
                        .on_press(Msg::FormatoCelula(Evento::CorPredefinida(hex)))
                );
            }
            paleta=paleta.push(linha);
        }
        let cor=if seletor_cor::hex_valido(&e.cor_fundo) {
            let [r,g,b,a]=seletor_cor::rgba_de(&e.cor_fundo);
            pelicano::Color::rgb8(r, g, b).alpha(a as f32/255.)
        }else{pelicano::Color::WHITE};
        let contraste=if (cor.r*0.299+cor.g*0.587+cor.b*0.114)<0.52 {
            pelicano::Color::WHITE
        }else{pelicano::Color::rgb8(30,33,36)};
        let nome = if e.cor_fundo.is_empty() { tr!("Sem preenchimento").to_owned() } else { e.cor_fundo.clone() };
        let esquerda=pelicano::column![
            secao(tr!("PALETA DE CORES")),
            paleta,
            pelicano::row![
                swatch(&e.cor_fundo,18.,18.),
                text(nome).size(12).width(Length::FILL),
                button(pelicano::row![feather_ui::icones::icone("eraser",14.), text(tr!("Sem preenchimento")).size(12)]
                    .spacing(6).align(pelicano::Align::Center))
                    .padding([5,9]).style(estilo_botao_secundario)
                    .on_press(Msg::FormatoCelula(Evento::Texto(TextoCampo::CorFundo,String::new()))),
            ].spacing(8).align(pelicano::Align::Center),
            button(pelicano::row![feather_ui::icones::icone("palette",14.), text(tr!("Mais cores…")).size(12).no_wrap()]
                .spacing(6).align(pelicano::Align::Center))
                .padding([5,9]).style(estilo_botao_pequeno(e.mais_cores))
                .on_press(Msg::FormatoCelula(Evento::MaisCores)),
        ].spacing(10).width(Length::FILL);
        let direita: Element<'_, Msg> = if e.mais_cores {
            seletor_no_dialogo(e,tr!("Preenchimento"))
        } else {
            pelicano::column![
                secao(tr!("PRÉVIA")),
                container(text(tr!("Aa 123")).size(18).color(contraste))
                    .width(Length::FILL).align_x(pelicano::Align::Center).height(Length::Fixed(150.)).align_y(pelicano::Align::Center)
                    .style(move |_|pelicano::Style {
                        background:Some(pelicano::Background::Color(cor)),
                        border:pelicano::Border{color:COR_BORDA(),width:1.,radius: [7.0; 4]},
                        ..Default::default()
                    }),
            ].spacing(8).into()
        };
        pelicano::row![
            container(esquerda).padding(12).width(Length::FILL).style(estilo_painel_faixa),
            container(direita).padding(12).width(Length::Fixed(300.)).style(estilo_painel_faixa),
        ].spacing(12).align(pelicano::Align::Start).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn abas_exibem_seletor_hsv_e_previa_de_borda() {
        let mut app=App::default();
        app.formato_celula=Some(Estado::novo(&app,Aba::Borda));
        let e=app.formato_celula.as_ref().unwrap();
        let _: Element<'_,Msg>=app.formato_borda(e);
        let _: Element<'_,Msg>=app.formato_preenchimento(e);
        // Com o seletor completo aberto as duas abas também montam.
        app.update(Msg::FormatoCelula(Evento::MaisCores));
        let e=app.formato_celula.as_ref().unwrap();
        assert!(e.mais_cores);
        let _: Element<'_,Msg>=app.formato_borda(e);
        let _: Element<'_,Msg>=app.formato_preenchimento(e);
        let svg=desenho_previa(e);
        let _: Element<'_,Msg>=svg.into();
    }
}
