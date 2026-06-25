const canvas = document.getElementById('particleCanvas');
const ctx = canvas.getContext('2d', { willReadFrequently: true });
canvas.width = window.innerWidth;
canvas.height = window.innerHeight;

let particlesArray = [];
let particleSize = 2;
let resolution = 6; // Sample every 6th pixel for good density
let mouse = {
    x: null,
    y: null,
    radius: 120 // How far the explosion reaches
};

window.addEventListener('mousemove', function(event) {
    mouse.x = event.x;
    mouse.y = event.y;
});
window.addEventListener('mouseleave', function() {
    mouse.x = null;
    mouse.y = null;
});

let resizeTimer;
window.addEventListener('resize', function() {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
        if(image.complete) init(); // Reinitialize on resize
    }, 200);
});

const image = new Image();
image.src = 'headphone.png';

class Particle {
    constructor(x, y, color) {
        // Start scattered around the screen for an assembly animation
        this.x = x + (Math.random() * 1000 - 500); 
        this.y = y + (Math.random() * 1000 - 500);
        this.baseX = x;
        this.baseY = y;
        this.color = color;
        this.size = particleSize;
        this.density = (Math.random() * 40) + 5; // Return speed variability
        this.z = Math.random() * 1.5 + 0.5; // Simulate depth size variation
    }

    draw() {
        ctx.fillStyle = this.color;
        ctx.beginPath();
        ctx.arc(this.x, this.y, this.size * this.z, 0, Math.PI * 2);
        ctx.closePath();
        ctx.fill();
    }

    update() {
        let dx = mouse.x - this.x;
        let dy = mouse.y - this.y;
        let distance = Math.sqrt(dx * dx + dy * dy);
        let forceDirectionX = dx / distance;
        let forceDirectionY = dy / distance;
        let maxDistance = mouse.radius;
        let force = (maxDistance - distance) / maxDistance;
        let directionX = forceDirectionX * force * this.density;
        let directionY = forceDirectionY * force * this.density;

        if (distance < maxDistance) {
            // Push away (explosion effect)
            this.x -= directionX;
            this.y -= directionY;
        } else {
            // Smoothly return to original position
            if (this.x !== this.baseX) {
                let dx = this.x - this.baseX;
                this.x -= dx / 15;
            }
            if (this.y !== this.baseY) {
                let dy = this.y - this.baseY;
                this.y -= dy / 15;
            }
        }
    }
}

function init() {
    particlesArray = [];
    
    // Fit the image dynamically based on screen size
    const imgRatio = image.width / image.height;
    const screenRatio = canvas.width / canvas.height;
    
    let drawHeight = canvas.height * 0.7; // take up 70% of screen height
    let drawWidth = drawHeight * imgRatio;
    
    // On small screens, shrink it further
    if (drawWidth > canvas.width * 0.5) {
        drawWidth = canvas.width * 0.5;
        drawHeight = drawWidth / imgRatio;
    }
    
    // Position it on the right side
    const startX = canvas.width * 0.7 - (drawWidth / 2);
    const startY = (canvas.height / 2) - (drawHeight / 2);

    const offCanvas = document.createElement('canvas');
    const offCtx = offCanvas.getContext('2d', { willReadFrequently: true });
    offCanvas.width = canvas.width;
    offCanvas.height = canvas.height;
    
    offCtx.drawImage(image, startX, startY, drawWidth, drawHeight);
    const imageData = offCtx.getImageData(0, 0, offCanvas.width, offCanvas.height);
    const data = imageData.data;

    // Build particle array
    for (let y = 0; y < canvas.height; y += resolution) {
        for (let x = 0; x < canvas.width; x += resolution) {
            const index = (y * canvas.width + x) * 4;
            const r = data[index];
            const g = data[index + 1];
            const b = data[index + 2];
            const alpha = data[index + 3];
            
            // Filter out the pure black background
            // If the sum of RGB is > 20, it's not black background
            if (alpha > 128 && (r + g + b > 25)) {
                // enhance color slightly for better vibrancy
                const color = `rgba(${Math.min(r+20, 255)},${Math.min(g+20, 255)},${Math.min(b+20, 255)},${alpha/255})`;
                particlesArray.push(new Particle(x, y, color));
            }
        }
    }
}

function animate() {
    requestAnimationFrame(animate);
    // Use a slight opacity for trailing effect
    ctx.fillStyle = 'rgba(0, 0, 0, 0.2)'; 
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    
    for (let i = 0; i < particlesArray.length; i++) {
        particlesArray[i].draw();
        particlesArray[i].update();
    }
}

image.onload = function() {
    init();
    animate();
}
