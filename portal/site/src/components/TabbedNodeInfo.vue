<script>
    import info from './Info'
	import network from './Network'
    import peers from './Peers'
    import mempool from './Mempool'

    export default {
        name: "tabbed-node-info",
        components: {
            info,
			network,
            peers,
            mempool
        },
        data: function() {
            return {
                currentTab: "info",
                tabs: ["Info", "Network", "Peers", "Mempool"]
            }
        },
        computed: {
            currentTabComponent: function() {
                return this.currentTab.toLowerCase();
            }
        }
    }
</script>

<template>
    <div id="dynamic-component-demo" class="demo">
        <button
                v-for="tab in tabs"
                v-bind:key="tab"
                v-bind:class="['tab-button', { active: currentTab === tab }]"
                v-on:click="currentTab = tab"
        >
            {{ tab }}
        </button>

        <component v-bind:is="currentTabComponent" class="tab"/>
    </div>
</template>
