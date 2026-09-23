package org.slatelauncher.client.config

public object ConfigReconciler {
    public fun applyPatch(
        current: ConfigDocument,
        schema: ConfigSchema,
        expectedRevision: Long,
        patch: Map<String, ConfigValue>,
    ): ReconciliationResult {
        require(current.moduleId == schema.moduleId) { "configuration belongs to another module" }
        if (current.revision != expectedRevision) {
            throw RevisionConflictException(expectedRevision, current.revision)
        }

        val fields = schema.fields.associateBy { it.key }
        for ((key, value) in patch) {
            val field = requireNotNull(fields[key]) { "configuration field $key is not editable" }
            require(field.kind == value.kind) { "configuration field $key has the wrong type" }
        }

        val input = current.values + patch
        val defaulted = mutableSetOf<String>()
        val reconciled = linkedMapOf<String, ConfigValue>()
        for (field in schema.fields) {
            val value = input[field.key]
            if (value != null && value.kind == field.kind) {
                reconciled[field.key] = value
            } else {
                reconciled[field.key] = field.defaultValue
                defaulted += field.key
            }
        }

        val unknown = input.keys - fields.keys
        if (schema.preserveUnknownFields) {
            for (key in unknown.sorted()) {
                reconciled[key] = requireNotNull(input[key])
            }
        }

        return ReconciliationResult(
            document =
                ConfigDocument(
                    moduleId = current.moduleId,
                    schemaVersion = schema.version,
                    revision = current.revision + 1,
                    values = reconciled,
                ),
            defaultedFields = defaulted,
            preservedUnknownFields = if (schema.preserveUnknownFields) unknown else emptySet(),
        )
    }
}
