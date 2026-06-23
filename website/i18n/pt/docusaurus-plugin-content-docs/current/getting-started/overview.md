---
title: "Visão Geral"
description: "O que é o Campbooks e como funciona"
sidebar_position: 1
---

O Campbooks e um cliente de email nativo de IA com codigo fonte disponivel para profissionais e pequenas empresas. Lê os seus emails e documentos, usa IA para arquivar e destacar o que importa, e oferece um fluxo de trabalho claro de revisão e aprovação — reinventado para não se parecer em nada com o email a que está habituado.

## O que o Campbooks faz

- **Ingere emails** do Zoho Mail, Google Workspace ou Microsoft 365 via OAuth
- **Classifica documentos** com recurso a IA — faturas, contratos, recibos e muito mais
- **Prioriza itens de ação** — sabe o que precisa da sua atenção agora
- **Gere aprovações** — reveja, aprove ou rejeite documentos
- **Exporta para as suas ferramentas** — Google Drive, Zoho WorkDrive, Notion

## Como funciona

1. **Ligue uma conta de email** via OAuth. O Campbooks analisa a sua caixa de entrada em busca de emails com anexos.
2. **A IA classifica** cada anexo — reconhecendo tipos de documentos como faturas, contratos, recibos e mais.
3. **Os documentos aparecem no seu painel** com estados e itens de ação. Pode rever, aprovar ou exportá-los.
4. **A integração de email** permite-lhe responder, etiquetar e organizar emails diretamente a partir do Campbooks.

## Arquitetura

O Campbooks é uma aplicação Ruby on Rails com:

- **PostgreSQL** para a base de dados
- **Solid Queue** para processamento de tarefas em segundo plano
- **Tailwind CSS** para a interface
- **Hotwire** para funcionalidades interativas
- **Claude (Anthropic)** para análise e classificação de documentos com IA

## Próximos passos

- [Instalar o Campbooks](/docs/getting-started/installation) no seu servidor
- [Ligar uma conta de email](/docs/email/connecting-accounts)
- [Configurar os serviços de IA](/docs/ai/configuration)
