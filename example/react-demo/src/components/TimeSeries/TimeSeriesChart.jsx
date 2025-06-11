import React, { useEffect, useRef, useState, useCallback } from 'react';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js';
import { Line, Bar, Scatter } from 'react-chartjs-2';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend
);

// Static colors array to avoid re-creation on each render
const CHART_COLORS = [
  '#FF6384', '#36A2EB', '#FFCE56', '#4BC0C0',
  '#9966FF', '#FF9F40', '#E74C3C', '#2ECC71'
];

const TimeSeriesChart = ({ 
  historyData, 
  derivativeData,
  selectedKeys, 
  chartType = 'line', 
  timeRange = 'all',
  showDerivatives = true,
  onClose 
}) => {
  const chartRef = useRef(null);
  const [chartData, setChartData] = useState({ datasets: [] });

  const prepareChartData = useCallback(() => {
    if (!historyData || selectedKeys.size === 0) {
      setChartData({ datasets: [] });
      return;
    }

    const datasets = [];
    let colorIndex = 0;

    Array.from(selectedKeys).forEach(key => {
      if (historyData[key] && historyData[key].length > 0) {
        let data = historyData[key];
        
        // Apply time range filter
        if (timeRange !== 'all') {
          const limit = parseInt(timeRange.replace('last', ''));
          data = data.slice(-limit);
        }

        const baseColor = CHART_COLORS[colorIndex % CHART_COLORS.length];

        // Create original data dataset
        const dataset = {
          label: key,
          data: data.map((value, index) => ({ x: index, y: value })),
          borderColor: baseColor,
          backgroundColor: baseColor + '20',
          tension: chartType === 'line' ? 0.4 : 0,
          fill: false,
          pointRadius: chartType === 'scatter' ? 4 : 2,
          pointHoverRadius: 6,
          borderWidth: 2
        };
        
        datasets.push(dataset);

        // Add derivative dataset if available and enabled
        if (showDerivatives && derivativeData && derivativeData[key] && derivativeData[key].length > 0) {
          let derivData = derivativeData[key];
          
          // Apply same time range filter to derivatives
          if (timeRange !== 'all') {
            const limit = parseInt(timeRange.replace('last', ''));
            derivData = derivData.slice(-limit);
          }

          const derivativeDataset = {
            label: `${key} (derivative)`,
            data: derivData.map((value, index) => ({ x: index, y: value })),
            borderColor: baseColor,
            backgroundColor: baseColor + '10',
            tension: chartType === 'line' ? 0.2 : 0,
            fill: false,
            pointRadius: chartType === 'scatter' ? 3 : 1,
            pointHoverRadius: 4,
            borderWidth: 1,
            borderDash: [5, 5], // Dashed line for derivatives
            yAxisID: 'y1' // Use secondary y-axis for derivatives
          };
          
          datasets.push(derivativeDataset);
        }
        
        colorIndex++;
      }
    });

    setChartData({ datasets });
  }, [selectedKeys, chartType, timeRange, showDerivatives]);
  // }, [historyData, derivativeData, selectedKeys, chartType, timeRange, showDerivatives]);


  useEffect(() => {
    prepareChartData();
  }, [prepareChartData]);

  const chartOptions = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: {
      mode: 'index',
      intersect: false
    },
    plugins: {
      title: {
        display: true,
        text: showDerivatives ? 'Animation Time Series Data (with Derivatives)' : 'Animation Time Series Data',
        font: { size: 16 }
      },
      legend: {
        display: true,
        position: 'top'
      },
      tooltip: {
        callbacks: {
          title: function(context) {
            return `Sample: ${context[0].parsed.x}`;
          },
          label: function(context) {
            const isDerivative = context.dataset.label.includes('(derivative)');
            const value = context.parsed.y.toFixed(3);
            const unit = isDerivative ? '/s' : '';
            return `${context.dataset.label}: ${value}${unit}`;
          }
        }
      }
    },
    scales: {
      x: {
        type: 'linear',
        display: true,
        title: {
          display: true,
          text: 'Sample Index'
        },
        grid: {
          color: '#e0e0e0'
        }
      },
      y: {
        type: 'linear',
        display: true,
        position: 'left',
        title: {
          display: true,
          text: 'Value'
        },
        grid: {
          color: '#e0e0e0'
        }
      },
      y1: showDerivatives ? {
        type: 'linear',
        display: true,
        position: 'right',
        title: {
          display: true,
          text: 'Rate of Change (per sample)'
        },
        grid: {
          drawOnChartArea: true,
        },
      } : undefined
    },
    elements: {
      point: {
        radius: chartType === 'scatter' ? 4 : 2
      }
    },
    animation: {
      duration: 300
    }
  };

  const renderChart = () => {
    switch (chartType) {
      case 'bar':
        return <Bar ref={chartRef} data={chartData} options={chartOptions} />;
      case 'scatter':
        return <Scatter ref={chartRef} data={chartData} options={chartOptions} />;
      case 'line':
      default:
        return <Line ref={chartRef} data={chartData} options={chartOptions} />;
    }
  };

  return (
    <div className="chart-container">
      <div className="chart-header">
        <h3>Time Series Chart</h3>
        <button className="btn-secondary" onClick={onClose}>
          Close
        </button>
      </div>
      <div className="chart-wrapper">
        {chartData.datasets.length === 0 ? (
          <div className="no-chart-data">
            No data to display. Select keys and ensure history is being captured.
          </div>
        ) : (
          renderChart()
        )}
      </div>
      {showDerivatives && (
        <div className="chart-legend">
          <div className="legend-item">
            <span className="legend-line solid"></span>
            <span>Original Values</span>
          </div>
          <div className="legend-item">
            <span className="legend-line dashed"></span>
            <span>Derivatives (Rate of Change)</span>
          </div>
        </div>
      )}
    </div>
  );
};

export default TimeSeriesChart;
