# 服务层迁移交接

现有TypeScript服务仍位于`apps/web/src/server/services`，作为运行实现与行为回归参考。新产品服务边界见 [ARCHITECTURE](ARCHITECTURE.md)，原始接口与历史证据见 git 历史（提交 46f499a）。

迁移保留：依赖注入、纯领域逻辑、仓储事务、业务事件、每次模型调用计量、命令幂等、查词上下文、失败时的配额预留。需要改变：摆脱Next服务端绑定、平台IO独立、UTF-16到UTF-8位置转换、音频配置维度缓存、持久化任务与设备能力接口。

当前所有Rust服务未实现。依赖与具体交付从 [BACKLOG](BACKLOG.md) 领取；表结构见 [DATA_MODEL](DATA_MODEL.md)，旧数据转换见 [MIGRATION](MIGRATION.md)。旧接口中的`server/browser`字面值不直接作为原生UI文案。
