package org.slatelauncher.client.adapter.fabric.ui

import net.minecraft.network.chat.Component
import net.minecraft.resources.ResourceLocation

internal object SlateTypography {
    val uiFont: ResourceLocation = ResourceLocation.fromNamespaceAndPath("slate-client", "ui")
    val monoFont: ResourceLocation = ResourceLocation.fromNamespaceAndPath("slate-client", "mono")
    val brandFont: ResourceLocation = ResourceLocation.fromNamespaceAndPath("slate-client", "brand")

    fun ui(component: Component): Component = component.copy().withStyle { style ->
        style.withFont(uiFont)
    }

    fun mono(component: Component): Component = component.copy().withStyle { style ->
        style.withFont(monoFont)
    }

    fun brand(component: Component): Component = component.copy().withStyle { style ->
        style.withFont(brandFont)
    }
}
