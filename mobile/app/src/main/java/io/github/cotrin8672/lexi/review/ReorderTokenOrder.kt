package io.github.cotrin8672.lexi.review

data class ReorderBankSlot(
    val token: String,
    val selected: Boolean,
)

fun reorderBankSlots(bankOrder: List<String>, selectedTokens: List<String>): List<ReorderBankSlot> {
    val selectedCounts = selectedTokens.groupingBy { it }.eachCount().toMutableMap()
    return bankOrder.map { token ->
        val remaining = selectedCounts[token] ?: 0
        if (remaining > 0) {
            selectedCounts[token] = remaining - 1
            ReorderBankSlot(token = token, selected = true)
        } else {
            ReorderBankSlot(token = token, selected = false)
        }
    }
}

fun availableReorderTokens(bankOrder: List<String>, selectedTokens: List<String>): List<String> =
    reorderBankSlots(bankOrder, selectedTokens)
        .filterNot { it.selected }
        .map { it.token }

fun reorderAnswerSlotCount(bankOrder: List<String>): Int = bankOrder.size
