//! "O que há de novo": abre sozinha na primeira abertura depois de atualizar
//! (e em Ajuda ▸ O que há de novo). Segue a F1 (GUIA-JANELAS.md): lateral com
//! as versões, cabeçalho com a marca, cartões dos destaques e a lista do
//! resto. O conteúdo é embutido no app; nada é buscado na internet.
use super::*;
use feather_ui::consulta::{self, Ilustracao};

/// Uma versão publicada e o que ela trouxe.
pub struct Versao {
    pub numero: &'static str,
    pub data: &'static str,
    pub resumo: &'static str,
    /// Figura do cabeçalho da versão.
    pub ilustracao: Ilustracao,
    /// Cartões: ícone, título, descrição curta (até ~84 caracteres).
    pub destaques: &'static [(&'static str, &'static str, &'static str)],
    /// "Também nesta versão": melhorias menores e correções.
    pub tambem: &'static [&'static str],
}

/// Da mais nova para a mais antiga. A primeira é a desta compilação.
pub const VERSOES: &[Versao] = &[
    Versao {
        numero: "0.1.3",
        data: "19/10/2026",
        resumo: "Excel e acabamento",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("link", "Vínculo com outro arquivo no .xlsx", "O Excel abre sem reparo e mostra os últimos valores da origem."),
            ("math-function", "Fórmulas novas no Excel sem #NOME?", "AGREGAR, MUNIT, REGEX e PERCENTOF chegam ao Excel com o nome certo."),
            ("clipboard", "Colar data até o fim da coluna", "A data vai até a última linha com o formato junto, num Ctrl+Z só."),
            ("pointer", "Clique com a caneta da mesa", "A ponta clica em botões, células e menus, sem pedir compartilhar a tela."),
            ("photo", "Imagem entra e sai da célula", "Botão direito: Colocar sobre as células ou Colocar na célula."),
            ("table", "Fórmulas corrigidas", "SOMARPRODUTO com matriz, DIA.DA.SEMANA com o tipo e nome de outra aba em SOMASES."),
            ("code", "Macros com laço grande", "Laços de até 20 milhões de voltas; o erro de laço sem fim sai em português."),
            ("bolt", "Borda do Ctrl+C animada", "A área copiada fica com o traço andando, como no Excel."),
            ("palette", "Formatar células mais compacto", "A aba Alinhamento cabe inteira; códigos de número sem rolagem."),
        ],
        tambem: &[
            "Tabela dinâmica gravada no .xlsx é reconhecida pelo Excel como dinâmica.",
            "Barra de status: as contas ficam junto do zoom e alinhadas.",
            "Caixa de nome mostra o nome definido da seleção e uma célula só sem repetir (G17).",
            "Formatação condicional sem o código hexa da cor à mostra.",
            "Seletor de tipo de gráfico abre no centro, sem opções em cinza.",
            "Editor de macros: divisórias da lateral e da Janela Imediata arrastam direito.",
            "Macros: toLocaleString(\"pt-BR\") segue a localidade.",
            "Configurações gravadas sem risco de perder tudo numa queda.",
            "MCP: sheet_tela mostra os painéis abertos; parâmetros e mensagens consistentes.",
            "Menu ▾ da AutoSoma mostra o atalho Alt+=.",
            "Bloquear seleção aparece aceso quando a célula está bloqueada (o clique desbloqueia).",
        ],
    },
    Versao {
        numero: "0.1.2",
        data: "18/10/2026",
        resumo: "Todas as funções do Excel, faixa completa e menos travadas",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("math-function", "Todas as 520 funções do Excel", "DESLOC, INDIRETO, LAMBDA, PROCX… e todas nos menus da aba Fórmulas."),
            ("bolt", "Apagar e desfazer sem travar", "Apagar uma fórmula e dar Ctrl+Z: de 3,4 s para um instante."),
            ("device-floppy", "Salvar arquivo enorme mais rápido", "Ctrl+S em 13,7 milhões de linhas: a tela para 0,7 s em vez de 3,8 s."),
            ("chart-line", "Rolar com gráficos sem tranco", "O gráfico é desenhado uma vez; a rolagem só recorta a imagem pronta."),
            ("chart-bar", "Gráfico com um clique", "Colunas, Linhas, Pizza… direto da faixa; Recomendados escolhe pelos dados."),
            ("printer", "Várias áreas de impressão", "A prévia mostra todas, uma por página; cabeçalho e rodapé próprios."),
            ("text-box", "Caixa de texto", "Inserir › Caixa de texto: clique ou arraste e já comece a digitar."),
            ("bolt", "Abrir .xlsx mais rápido", "As abas e os gráficos do arquivo são lidos em paralelo."),
            ("omega", "Símbolos e equações", "Moedas, setas, frações, letras gregas e sinais de conta num clique."),
        ],
        tambem: &[
            "Cópia de recuperação de uma pasta com muitos erros: de 0,64 s para 0,13 s.",
            "LET e LAMBDA: variável com intervalo funciona dentro de SOMA e das outras funções de lista.",
            "Funções que dependem de serviço de fora (cubo, RTD, STOCKHISTORY) dizem o motivo do #N/D.",
            "Macros › Componentes abre o editor no componente HTML/CSS.",
            "Formas: o botão Efeitos liga e desliga a sombra.",
            "Ferramentas › Proteção: Bloquear seleção e Proteger planilha.",
            "Inserir › Impressão ganhou Adicionar à área.",
            "Texto da forma fica escuro sobre preenchimento claro.",
            "Erro repetido numa coluna inteira ocupa uma entrada só no arquivo.",
        ],
    },
    Versao {
        numero: "0.1.1",
        data: "18/10/2026",
        resumo: "Milhões de linhas sem travar, imagem na célula e menos memória",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("pivot-table", "Tabela dinâmica em segundo plano", "Em milhões de linhas a janela não para: a barra embaixo mostra o andamento."),
            ("bolt", "Fórmula na coluna inteira", "Preencher ou colar em 13,7 milhões de linhas leva milissegundos."),
            ("bolt", "MÍNIMOSES e os *SES sem travar", "Com A:A e B:B na fórmula: preencher mil linhas caiu de 76 s para 12 ms."),
            ("table", "Ctrl+T e região atual instantâneos", "Em 13 milhões de linhas a área da tabela sai em 0,1 s (antes travava)."),
            ("arrows-horizontal", "Inserir e excluir coluna rápido", "Com milhares de fórmulas: de 3,3 s para 0,03 s, referências certas."),
            ("device-floppy", "Arquivo enorme com menos memória", "Abrir 13,7 milhões de linhas: de 13,3 GB para 4,5 GB; salvar: de 20 para 9 GB."),
            ("photo", "Imagem dentro da célula", "Inserir na célula, tirar dela, Alt+arrastar; GIF, BMP e WebP aparecem."),
            ("palette", "Painel Formatar imagem", "Altura, largura em cm, giro, escala, proporção e mover com as células."),
            ("terminal", "IA cria tabela dinâmica", "Pelo MCP: dinâmica, formatação condicional e fórmulas sem limite de linhas."),
        ],
        tambem: &[
            "Tabela dinâmica: célula vazia e texto não entram mais como 0 em Média, Desvio padrão, Mínimo e Moda.",
            "Tabela dinâmica: mudar os campos reaproveita a origem já lida e refaz só a conta.",
            "Colar uma fórmula desloca as referências, como no Excel (=A1 colada 4 linhas abaixo vira =A5).",
            "Inserir ou excluir coluna ajusta referências de coluna inteira, como B:B.",
            "PARÂMETRO compara texto e número como o Excel.",
            "Colar milhões de fórmulas não dá mais um tranco a cada segundo.",
            "Ctrl+Z e Ctrl+Y de muitas células de uma vez ficaram mais rápidos.",
            "Gráfico a partir de uma célula numa tabela enorme não trava mais.",
            "Monitor (F12): avisa quando a tela fica parada mais de 200 ms e mostra o tempo do Ctrl+T.",
            "Figuras do .xlsx não passam mais por cima dos cabeçalhos ao rolar.",
            "A barra de fórmulas não mostra o código da imagem guardada na célula.",
            "Botão direito na imagem abre o menu dela; com a célula em edição, abre o menu da célula.",
            "Um painel lateral por vez: gráfico, tabela dinâmica ou imagem.",
            "Menu Orientação do texto com as setas certas.",
            "Exportar CSV ou TSV sugere o nome do arquivo.",
            "Pasta nova em inglês começa em Sheet1; em espanhol, Hoja1.",
            "O arquivo .fshz grande é gravado em partes, sem montar tudo na memória.",
        ],
    },
    Versao {
        numero: "0.1.0",
        data: "17/10/2026",
        resumo: "Tabela dinâmica rápida, referência entre arquivos e espanhol",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("pivot-table", "Tabela dinâmica muito mais rápida", "Trocar um campo em 1 milhão de linhas leva ~0,2 s; criar é na hora."),
            ("link", "Referência entre arquivos", "=[Vendas.fshz]Dados!A1 fica viva com os dois abertos; clique na outra janela."),
            ("clipboard", "Colar em 100 milhões de linhas", "Um valor colado na coluna inteira vai até a última linha em menos de 1 s."),
            ("bolt", "CSV grande ~10× mais rápido", "653 MB e 70 milhões de células abrem em 3,2 s com 2 núcleos; antes, 29,5 s."),
            ("bolt", ".xlsx com fórmulas ~4× mais rápido", "A fórmula repetida é traduzida uma vez só: de 33,7 s para 8,4 s com 2 núcleos."),
            ("world", "Interface em espanhol", "Terceiro idioma, com as funções em espanhol; abre no idioma do sistema."),
            ("chart-pivot", "Mostrar valores como, gráfico e detalhes", "% do total, acumulado e mais; gráfico dinâmico; duplo clique abre as linhas."),
            ("terminal", "IA pelo MCP", "Ligue em Configurações: a IA ordena, filtra, substitui, move e copia."),
            ("code", "Eventos da planilha no Alt+F11", "Código ao mudar células e ao ativar a aba, pastas de módulos e divisórias."),
        ],
        tambem: &[
            "Tabela dinâmica: nasce vazia, como no Excel, e mostra \"Atualizando…\" na barra em origens grandes.",
            "Tabela dinâmica: aba própria na faixa, filtro de relatório na grade e arrastar para reordenar os campos.",
            "Tabela dinâmica: Soma, Média, Desvio, Moda… ao clicar no campo; duplo clique em 125 mil linhas em 0,4 s.",
            "Segmentação de dados e atualizar sozinha quando a origem muda.",
            "Ctrl+C marca a área copiada com borda tracejada, até Esc ou uma edição.",
            "Barra de status do seu jeito: botão direito escolhe média, contagem, mín, máx, soma e mais.",
            "Gráfico com milhões de linhas resume os dados (soma, média, contagem…) em vez de travar.",
            "Fórmula colada numa coluna inteira para na última linha com dados (antes travava).",
            "O gráfico novo aparece na parte visível da planilha.",
            "O menu do botão direito no gráfico não some mais ao passar o mouse.",
            "Somar dias a uma data (=A1+2) mantém o formato de data.",
            "Um painel lateral por vez (gráfico ou tabela dinâmica).",
            "Ocultar linhas ou colunas até o fim da planilha.",
            "Abrir pela tela inicial mostra o carregamento.",
            "feathersheet --mcp liga o servidor MCP sem instalar nada a mais.",
            "A interface em inglês sem sobras em português (borda, console F12, Configurações).",
            "No Windows, o app abre sem a janela de console (cmd) junto.",
            "No Windows, o monitor (F12) mostra a memória e o processador.",
            "Imagem dentro da célula: a fórmula IMAGEM e as figuras \"Colocar na célula\" do Excel aparecem.",
            "Várias áreas de impressão por aba, cada uma nas suas páginas.",
            "AutoSoma (e Média, Contagem, Máximo, Mínimo) já grava o resultado, sem esperar o Enter.",
            "Tab durante a edição confirma e anda para a direita.",
            "Excluir a primeira linha de um intervalo encolhe o intervalo, como no Excel (antes virava #REF!).",
            "Gantt com datas mostra as datas no eixo.",
            "Contar distintos nas séries de texto do gráfico, e trocar a categoria sem travar.",
        ],
    },
    Versao {
        numero: "0.0.5",
        data: "16/10/2026",
        resumo: "Interface em inglês, Layout da Página e .xlsx muito mais rápido",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("world", "Interface em inglês", "Menus, faixa, janelas, Ajuda e fórmulas mudam de idioma em Configurações."),
            ("printer", "Aba Layout da Página", "Margens, orientação, tamanho, área de impressão, quebras e escala na faixa."),
            ("palette", "Formatar células refeito", "Seis abas: número, alinhamento, fonte, borda, preenchimento e proteção."),
            ("bolt", "Abre .xlsx 3,6× mais rápido", "A Planilha Pesada (7 milhões de fórmulas) fica pronta em 9,7 s, antes 34 s."),
            ("table", "Somar e contar por cor", "SOMAR.POR.COR e CONT.SE.COR pela cor do fundo ou da fonte da célula."),
            ("chart-bar", "Dica do mouse por tipo", "Gantt mostra início, término e duração; velas, os quatro preços; cascata, o acumulado."),
        ],
        tambem: &[
            "Funções sem nome em português ganharam um (MULT, TETO e outras); o autocompletar segue o idioma.",
            "Seus dados (nome, iniciais e organização) em formulário alinhado.",
            "Delete apaga a forma ou o gráfico selecionado; a barra de fórmulas não fica com o foco.",
            "Barra de progresso por etapas ao abrir arquivos grandes.",
            "Menos memória ao abrir pastas grandes.",
            "O monitor (F12) e a barra de baixo também em inglês.",
        ],
    },
    Versao {
        numero: "0.0.4",
        data: "16/10/2026",
        resumo: "Macros dentro do arquivo e novidades ao atualizar",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("sparkles", "O que há de novo ao atualizar", "Quem vem de uma versão anterior vê esta janela na primeira abertura."),
            ("rocket", "Primeiros passos na instalação", "O assistente abre sozinho só numa instalação nova; depois, no menu Ajuda."),
            ("code", "Macros dentro do arquivo", "Cada planilha guarda as suas macros no .fshz; uma nova começa sem nenhuma."),
        ],
        tambem: &[
            "Na 0.0.3, quem atualizava da 0.0.2 recebia os Primeiros passos em vez das novidades.",
            "Atribuir macro e o editor (Alt+F11) não mostram mais macros de outros arquivos.",
        ],
    },
    Versao {
        numero: "0.0.3",
        data: "16/10/2026",
        resumo: "Gráficos no visual novo e recuperação automática",
        ilustracao: Ilustracao::Grafico,
        destaques: &[
            ("chart-bar", "25 tipos de gráfico", "Todos no visual novo, inclusive combinado, cascata, radar, Gantt e mapa de calor."),
            ("pointer", "Dica do mouse", "Passe o mouse num ponto para ver o valor; o texto da dica é personalizável."),
            ("list", "Legenda clicável", "Clique numa série da legenda para ocultá-la, com transição suave."),
            ("chart-line", "Elementos analíticos", "Linha de tendência, barras de erro, máximo, mínimo e média."),
            ("history", "Recuperação automática", "Se o computador desligar, a planilha alterada volta pela tela inicial."),
            ("arrow-back-up", "Ctrl+Z nos gráficos", "Mover e redimensionar um gráfico agora se desfaz como qualquer edição."),
            ("rocket", "Primeiros passos", "Assistente da primeira abertura e esta janela de novidades."),
            ("user", "Seus dados", "Seu nome assina os comentários e vai nas propriedades do .fshz e do .xlsx."),
        ],
        tambem: &[
            "Shift + roda do mouse rola a planilha na horizontal.",
            "Painel de formatação do gráfico com grupos separados por divisórias.",
            "Gráficos importados do .xlsx sem preenchimento voltam a ter fundo.",
            "A barra de abertura do .parquet conta linhas, não megabytes.",
            "O rodapé (Contagem, Soma) não para mais no meio da seleção em pastas novas.",
        ],
    },
    Versao {
        numero: "0.0.2",
        data: "16/10/2026",
        resumo: "Atualização pelo app e grade mais leve",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("refresh", "Procurar atualização", "Em Configurações, Sobre: consulta a versão nova e instala à vista, num terminal."),
            ("table", "Grade mais leve", "O desenho da grade fica guardado e só é refeito quando algo muda."),
            ("device-desktop", "Nome e logo no sistema", "O monitor do sistema do Linux mostra o FeatherSheet com nome e ícone."),
        ],
        tambem: &[
            "O cursor só é acompanhado na janela principal, sem lentidão com outra janela aberta.",
            "Identificação da janela e ícone corrigidos.",
        ],
    },
    Versao {
        numero: "0.0.1",
        data: "15/10/2026",
        resumo: "Primeira versão pública",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("table", "Planilha nativa", "Motor e interface em Rust, rápidos com arquivos grandes."),
            ("math-function", "Funções de planilha", "Nomes em português e em inglês, com ajuda e exemplos na F1."),
            ("code", "Macros", "Editor de macros em JavaScript (Alt+F11) e importação de VBA."),
            ("device-floppy", "Instaladores", "Pacote .deb, .tar.gz e instalador do Windows."),
        ],
        tambem: &[
            "Repositório próprio, com a paridade com o Excel descrita no README.",
        ],
    },
];

#[derive(Debug, Clone)]
pub enum Evento {
    Versao(usize),
    VerNoGithub,
    Fechar,
}

impl App {
    pub(super) fn evento_novidades(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Versao(i) => self.novidades_versao = i.min(VERSOES.len() - 1),
            Evento::VerNoGithub => {
                let v = &VERSOES[self.novidades_versao.min(VERSOES.len() - 1)];
                if !cfg!(test) {
                    super::atualizacao::abrir_endereco(&format!("{}/tag/v{}", super::atualizacao::PAGINA_VERSOES, v.numero));
                }
            }
            Evento::Fechar => {
                if let Some(id) = self.janelas.id(Auxiliar::Novidades) {
                    return self.update(Msg::Fechar(id));
                }
            }
        }
        Task::none()
    }

    pub(super) fn view_novidades(&self) -> Element<'_, Msg> {
        use iced::widget::column;
        let atual = self.novidades_versao.min(VERSOES.len() - 1);
        let mut nav = column![consulta::rotulo_grupo(tr!("Versões"))].spacing(2);
        for (i, v) in VERSOES.iter().enumerate() {
            nav = nav.push(consulta::item_nav(trf!("Versão {}", v.numero),
                if i == 0 { "sparkles" } else { "history" }, i == atual,
                Some(Msg::Novidades(Evento::Versao(i)))));
        }
        let v = &VERSOES[atual];
        let instalada = v.numero == super::atualizacao::versao_atual();
        let mut tambem = column![].spacing(8);
        for item in v.tambem {
            tambem = tambem.push(row![
                feather_ui::icones::icone("check", 15.),
                text(tr!(*item)).size(13).line_height(1.4),
            ].spacing(9).align_y(Alignment::Start));
        }
        let conteudo = column![
            consulta::cabecalho(trf!("Versão {}", v.numero),
                format!("{} · {}{}", v.data, tr!(v.resumo), if instalada { tr!(" · instalada neste computador") } else { "" }),
                Some(consulta::ilustracao(v.ilustracao))),
            consulta::negrito(tr!("Destaques"), 18.),
            consulta::grade(v.destaques.iter()
                .map(|(icone, titulo, descricao)| consulta::cartao(icone, tr!(*titulo), tr!(*descricao), None))
                .collect(), 3),
            consulta::divisor(),
            consulta::negrito(tr!("Também nesta versão"), 18.),
            tambem,
        ].spacing(17);
        let rodape = row![
            Space::with_width(Length::Fill),
            button(row![feather_ui::icones::icone("link", 15.), text(tr!("Ver no GitHub")).size(12)]
                .spacing(6).align_y(Alignment::Center))
                .padding([8, 13]).style(consulta::estilo_cartao)
                .on_press(Msg::Novidades(Evento::VerNoGithub)),
            button(text("OK").size(12)).padding([8, 18]).style(estilo_botao_primario)
                .on_press(Msg::Novidades(Evento::Fechar)),
        ].spacing(10).align_y(Alignment::Center);
        consulta::janela(consulta::lateral(nav, None), conteudo.into(), Some(rodape.into()))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn versao_desta_compilacao_e_a_primeira_e_todas_desenham() {
        assert_eq!(VERSOES[0].numero, super::super::atualizacao::versao_atual(),
            "acrescente as novidades da versão nova em VERSOES");
        let mut app = App::default();
        for i in 0..VERSOES.len() + 1 {
            let _ = app.update(Msg::Novidades(Evento::Versao(i)));
            let _: Element<'_, Msg> = app.view_novidades();
        }
        assert_eq!(app.novidades_versao, VERSOES.len() - 1);
        for v in VERSOES {
            for (icone, _, descricao) in v.destaques {
                assert!(feather_ui::icones::existe(icone), "{icone}");
                assert!(descricao.chars().count() <= 90, "{descricao}");
            }
        }
    }
}
