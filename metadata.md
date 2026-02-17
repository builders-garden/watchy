{"type":"https://eips.ethereum.org/EIPS/eip-8004#registration-v1","name":"Meerkat James","description":"James is a specialized AI agent with deep expertise in robotics, automation, and intelligent systems. With comprehensive knowledge spanning mechanical engineering, control systems, computer vision, and embedded programming, James helps users navigate the complex world of robotics development. Meerkat Town.","image":"https://www.meerkat.town/meerkats/meerkat_019.png","services":[{"name":"MCP","endpoint":"https://meerkat.up.railway.app/mcp/meerkat-19","version":"2025-06-18","mcpTools":["chat","get_agent_info"],"mcpPrompts":["greeting","help"]},{"name":"A2A","endpoint":"https://meerkat.up.railway.app/agents/meerkat-19/.well-known/agent-card.json","version":"0.3.0","a2aSkills":["natural_language_processing/natural_language_generation/text_generation","natural_language_processing/natural_language_understanding/contextual_comprehension","tool_interaction/automation/workflow_automation","natural_language_processing/information_retrieval_synthesis/search","natural_language_processing/conversation/chatbot"]},{"name":"OASF","endpoint":"https://meerkat.up.railway.app/oasf/meerkat-19","version":"v0.8.0","skills":["natural_language_processing/natural_language_generation/text_generation","natural_language_processing/natural_language_understanding/contextual_comprehension","tool_interaction/automation/workflow_automation","natural_language_processing/information_retrieval_synthesis/search","natural_language_processing/conversation/chatbot"],"domains":["technology/artificial_intelligence/deep_learning","research_and_development/research/data_collection","research_and_development/data_science/experimentation","technology/software_engineering/apis_integration","technology/software_engineering/web_development"]},{"name":"web","endpoint":"https://meerkat.town"}],"registrations":[{"agentId":1434,"agentRegistry":"eip155:8453:0x8004A169FB4a3325136EB29fA0ceB6D2e539a432"}],"supportedTrust":["reputation","crypto-economic","tee-attestation"],"active":true,"x402support":true,"updatedAt":1771026827,"meerkatId":19,"pricePerMessage":"Free"}



{
    "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
    "name": "Minara AI",
    "description": "Intelligent crypto assistant powered by AI. Provides real-time market analysis, DeFi guidance, swap intent parsing, perp trading suggestions, and prediction market analysis. Supports x402 with multi-chain support. Website: https://minara.ai | X: @minara",
    "image": "https://minara.ai/images/minara-logo-lg.png",
    "x402support": true,
    "active": true,
    "services": [
      {
        "name": "A2A",
        "endpoint": "https://x402.minara.ai/.well-known/agent-card.json",
        "version": "0.3.0",
        "a2aSkills": [
          "natural_language_processing/information_retrieval_synthesis/question_answering",
          "natural_language_processing/analytical_reasoning/analytical_reasoning",
          "natural_language_processing/natural_language_understanding/intent_detection",
          "analytical_skills/forecasting"
        ]
      },
      {
        "name": "OASF",
        "endpoint": "https://github.com/agntcy/oasf/",
        "version": "v0.8.0",
        "skills": [
          "natural_language_processing/information_retrieval_synthesis/question_answering",
          "natural_language_processing/analytical_reasoning/analytical_reasoning",
          "natural_language_processing/natural_language_understanding/intent_detection",
          "analytical_skills/forecasting"
        ],
        "domains": [
          "technology/blockchain",
          "finance_and_business/finance",
          "finance_and_business/investment_services",
          "technology/blockchain/cryptocurrency",
          "technology/blockchain/defi"
        ]
      },
      {
        "name": "web",
        "endpoint": "https://minara.ai"
      },
      {
        "name": "twitter",
        "endpoint": "https://x.com/minara"
      },
      {
        "name": "email",
        "endpoint": "support@minara.ai"
      }
    ],
    "registrations": [
      {
        "agentId": 608,
        "agentRegistry": "eip155:11155111:0x8004a818bfb912233c491871b3d84c89a494bd9e"
      },
      {
        "agentId": 6888,
        "agentRegistry": "eip155:1:0x8004A169FB4a3325136EB29fA0ceB6D2e539a432"
      }
    ],
    "supportedTrust": [
      "reputation",
      "crypto-economic",
      "tee-attestation"
    ],
    "updatedAt": 1769757405
  }


  {
    "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
    "name": "Loopuman",
    "description": "The Human Layer for AI — routes tasks to verified human workers worldwide via Telegram & WhatsApp. AI agents send requests, humans complete them, quality-checked results returned with 8-second cUSD payments on Celo. Services: data labeling, content moderation, surveys, translation, human verification oracles.",
    "image": "https://api.loopuman.com/logo.png",
    "version": "4.2.0",
    "agentType": "service",
    "sourceCode": "https://github.com/seesayearn-boop/humanoracle",
    "documentation": "https://github.com/seesayearn-boop/humanoracle/blob/main/docs/API.md",
    "author": {
        "name": "Loopuman",
        "url": "https://loopuman.com",
        "twitter": "https://x.com/loopuman"
    },
    "license": "MIT",
    "tags": [
        "oracle",
        "microtasking",
        "human-in-the-loop",
        "data-labeling",
        "verification",
        "RLHF",
        "AI"
    ],
    "services": [
        {
            "name": "A2A",
            "version": "0.3.0",
            "endpoint": "https://api.loopuman.com/.well-known/agent-card.json",
            "a2aSkills": [
                "human:task",
                "human:bulk_tasks",
                "human:verification",
                "human:voice_task",
                "data:labeling",
                "content:moderation",
                "translation:multilingual",
                "survey:completion"
            ]
        },
        {
            "name": "MCP",
            "version": "2025-06-18",
            "endpoint": "https://api.loopuman.com/.well-known/mcp.json",
            "mcpTools": [
                "create_task",
                "check_task",
                "create_bulk_tasks",
                "get_results"
            ]
        },
        {
            "name": "OASF",
            "version": "1.0.0",
            "endpoint": "https://api.loopuman.com/.well-known/oasf.json",
            "skills": [
                {
                    "name": "natural_language_processing/text_generation",
                    "id": 301
                },
                {
                    "name": "natural_language_processing/text_completion",
                    "id": 302
                },
                {
                    "name": "natural_language_processing/summarization",
                    "id": 306
                },
                {
                    "name": "natural_language_processing/text_translation",
                    "id": 305
                },
                {
                    "name": "advanced_reasoning_planning/task_planning",
                    "id": 1001
                },
                {
                    "name": "agent_orchestration/agent_coordination",
                    "id": 1004
                },
                {
                    "name": "data_engineering/data_collection",
                    "id": 601
                },
                {
                    "name": "data_engineering/data_labeling",
                    "id": 602
                },
                {
                    "name": "data_engineering/data_transformation_pipeline",
                    "id": 603
                },
                {
                    "name": "data_engineering/data_quality_management",
                    "id": 604
                }
            ],
            "domains": [
                {
                    "name": "technology/data_science",
                    "id": 1601
                },
                {
                    "name": "technology/artificial_intelligence",
                    "id": 1602
                },
                {
                    "name": "finance_and_business/business_operations",
                    "id": 1701
                }
            ]
        },
        {
            "name": "Web",
            "version": "2.0.0",
            "endpoint": "https://loopuman.com"
        }
    ],
    "x402Support": true,
    "active": true,
    "registrations": [
        {
            "agentId": 17,
            "agentRegistry": "eip155:42220:0x8004A169FB4a3325136EB29fA0ceB6D2e539a432"
        }
    ],
    "supportedTrust": [
        "reputation"
    ]
}