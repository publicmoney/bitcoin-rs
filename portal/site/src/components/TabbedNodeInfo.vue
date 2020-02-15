<script>
    import info from './Info'
    import peers from './Peers'

    export default {
        name: "tabbed-node-info",
        components: {
            info,
            peers
        },
        data: function() {
            return {
                currentTab: "info",
                tabs: ["Info", "Peers"]
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