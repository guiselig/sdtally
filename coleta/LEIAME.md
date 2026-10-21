# sdc-coleta

Programa que mede de onde vem o "Dados do Sistema" do seu Mac e grava um relatório para
comparar com outros Macs. Ele **só lê**: não apaga, não move, não muda nenhum ajuste e não pede
senha de administrador. O único arquivo que ele cria é o próprio relatório.

Funciona em Mac com chip Apple (M1 ou mais novo) e macOS 15 Sequoia ou mais novo. Mac com
processador Intel não é suportado. Foi testado no macOS 27; em versões mais antigas, o que não existir aparece no
relatório como erro, sem travar, e isso ajuda o projeto.

**Roda em segundo plano e não atrapalha:** usa a prioridade mais baixa do macOS para processador e
disco (o sistema adia o trabalho enquanto você usa o Mac), não abre janelas, não mostra pedidos
de permissão e não baixa nada do iCloud. A primeira parte leva de 3 a 30 minutos. Depois ele
fica até 2 horas tirando amostras leves a cada 5 minutos e regravando o relatório a cada 15: você
pode continuar trabalhando e encerrar quando quiser, sem perder o que já foi gravado. Discos
externos e de rede não são lidos.

## O que ele coleta

- Modelo do Mac, chip, memória, versão do macOS e tamanho do disco.
- Tamanho usado de cada volume do disco (Sistema, Dados, Recuperação...), identificados pela função, nunca pelo nome.
- Os números da tela Ajustes > Geral > Armazenamento, se ela foi aberta pouco antes (o macOS guarda esse registro por menos de um dia).
- Tamanho dos snapshots locais do Time Machine (só tipo e data) e se há um disco de backup configurado (sim/não, local ou rede).
- Espaço "removível" que o macOS pode liberar sozinho, por serviço do sistema.
- Espaço preso em arquivos apagados que algum app ainda mantém aberto (com o nome do app).
- Uso de swap (memória que foi para o disco) e a data em que o Mac foi ligado.
- Histórico do Time Machine: data do último backup concluído e quantas tentativas houve depois
  (nunca o endereço nem o nome do disco de backup), e de quem é cada snapshot.
- O que o sdtally recomendaria para o seu Mac (só o tipo da recomendação e o tamanho, sem caminhos).
- A cada 5 minutos: espaço livre, espaço usado, swap e **quanto cada programa gravou no disco**
  nesse período (pelo nome do programa). **Atenção:** isso mostra quais programas você usou
  durante a coleta. Se não quiser mostrar algum, apague a linha dele antes de mandar.
- Tamanho de pastas, agrupadas por nomes genéricos. Exemplos: `~/Documents`, `~/Library/Caches/Google`, `/Applications/Xcode.app/Contents/Developer`.
  **Atenção:** isso mostra quais apps, jogos e ferramentas de programação você tem instalados
  (os que ocupam mais de 100 MB). Se não quiser mostrar algum, apague a linha dele no arquivo
  antes de mandar.

## O que ele NÃO coleta

- Nenhum nome de arquivo e nenhum conteúdo de arquivo.
- Pastas pessoais dentro de Documentos, Mesa, Downloads etc. aparecem só como `~/Documents`.
  Qualquer outra pasta sua na pasta de início aparece como `~/<pasta do usuário>`.
- Seu nome, nome de usuário, e-mail, Apple ID, nome do Mac, número de série, nomes de rede ou
  de discos. Pasta com `@` ou com seu nome vira `<privado>`, e códigos de conta viram `<uuid>`.
- Nada é enviado pela internet. Você decide se manda o arquivo.

## Antes de rodar

1. Abra **Ajustes > Geral > Armazenamento** e espere a barra colorida terminar de carregar.
   Assim o relatório traz os números que a tela mostra.
2. Dê **Acesso Total ao Disco** ao app onde você vai digitar o comando (o Terminal do Mac; se usar
   iTerm, Warp ou o terminal do VS Code, é esse app que entra na lista, não o `sdc-coleta`):
   Ajustes > Privacidade e Segurança > Acesso Total ao Disco > ligue o app (se não estiver na
   lista, botão +, pasta Aplicativos > Utilitários > Terminal). Depois feche o app com Cmd-Q e
   abra de novo: a permissão só vale para janelas abertas depois. Sem isso
   o programa não roda: sem esse acesso o macOS mostraria pedidos de permissão na tela no meio da
   coleta. Se esquecer, o próprio programa avisa e, com Enter, abre o Ajustes já nessa lista
   (nenhum programa consegue ligar o acesso sozinho; o macOS só deixa você fazer isso).
   **Desligue depois da coleta.**

## Como rodar

Dê dois cliques no `sdc-coleta.zip` (ele vira a pasta `sdc-coleta`). **Antes de abrir o
programa**, tire a marca de "baixado da internet": no Terminal, entre nessa pasta (se baixou em
outro lugar, troque o caminho) e rode, nesta ordem:

```sh
cd ~/Downloads/sdc-coleta
xattr -cr .                                   # tira a marca de "baixado da internet"
chmod +x ./sdc-coleta
./sdc-coleta                                  # pede a senha do Mac uma vez
```

Depois do `xattr`, também dá para abrir com dois cliques no programa `sdc-coleta`: o macOS abre o
Terminal e ele pede a senha do Mac.

**Recebeu num pendrive?** Não precisa do Terminal nem do `xattr`: abra o pendrive no Finder
e dê dois cliques no programa `sdc-coleta`. Ele abre o Terminal, pede
a senha do Mac (ao digitar, ela não aparece; aperte Enter) e começa. O relatório fica na Mesa do
Mac.

Se aparecer **"sdc-coleta está danificado e não pode ser aberto"**, clique em **Cancelar** (não
mande para o Lixo) e rode o `xattr -cr .` acima. O arquivo não está danificado: o macOS diz isso
de todo programa baixado que não tem a assinatura paga da Apple, e o `xattr` tira essa marca.

Quando aparecer "Agora ele fica medindo em segundo plano", o relatório já está salvo. Deixe o
Terminal aberto (pode minimizar) e continue trabalhando. Para encerrar antes das 2 horas, aperte
Ctrl-C ou feche o Terminal: ele grava o que mediu até ali e para. **Não abra, edite nem mova o
arquivo antes de aparecer "Revise o arquivo antes de mandar"** (ou de você encerrar): se mexer
antes, ele para de gravar para não passar por cima da sua edição. Enquanto roda, o programa impede o Mac de dormir sozinho (a tela
ainda apaga e bloqueia normalmente, o que não atrapalha a coleta). Se for notebook, deixe na
tomada e **não feche a tampa**: tampa fechada faz o Mac dormir de qualquer jeito, e a coleta
pausa até ele acordar.
`./sdc-coleta --minutos 30` encurta a parte das amostras; `--minutos 0` desliga.

A senha de administrador (o programa chama o `sudo` sozinho) é necessária para medir as pastas
do sistema e os arquivos presos por outros programas. O programa continua só lendo: não apaga
nem altera nada. `./sdc-coleta --sem-sudo` faz uma coleta incompleta, sem pedir senha.

No fim ele mostra um resumo e o caminho do relatório, que fica na Mesa:
`sdc-relatorio-AAAA-MM-DD-HHMM.json`.

Opções: `--saida arquivo.json` grava em outro lugar. `--minutos N` muda o tempo das amostras
(padrão 120, máximo 240). `--sem-xattr` fica ~30 % mais rápido, com uma medida a menos.

## Antes de mandar

Depois de aparecer "Revise o arquivo antes de mandar", abra o `.json` num editor de texto
(TextEdit serve) e confira se não há nada que você não
queira mostrar. O programa faz um autoteste no fim (`autoteste_privacidade`), mas **revise o
arquivo antes de mandar**.

## Como ler o resumo

- **usado do volume**: quanto o volume de dados ocupa.
- **árvore medida**: soma dos arquivos que o programa conseguiu ler (cópias contadas uma vez).
- **snapshots**: espaço preso em snapshots do Time Machine, que o macOS apaga sozinho com o tempo.
- **apagados e abertos**: arquivos já apagados que algum app ainda segura. Fechar o app libera.
- **pastas negadas**: pastas protegidas pelo sistema, medidas pelo próprio APFS sem abri-las
  (inclui os modelos do Apple Intelligence).
- **não atribuído**: o que sobra. Inclui metadados do sistema de arquivos e pastas que nem o
  APFS mede.
